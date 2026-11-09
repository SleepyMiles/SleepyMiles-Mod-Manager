//! Publishing mods to SleepyMiles' site (the second mod source), Thunderstore-style.
//!
//! The site is a static GitHub Pages site. A mod version is a GitHub Release on
//! `SleepyMiles/miles-lc-mods` tagged `<Name>-<version>` with `<Name>-<version>.zip` attached.
//! When that release is published, the repo's "Publish to mod site" workflow adds it to the
//! site's index. This module does the release part from inside the app, using a GitHub token
//! stored in the SleepyMiles slot of the token store.

use std::{
    io::Read,
    path::{Path, PathBuf},
};

use base64::{Engine, prelude::BASE64_STANDARD};
use eyre::{Context, Result, bail, eyre};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, command};
use tracing::{info, warn};

use crate::{
    state::ManagerExt,
    thunderstore::{Backend, token},
};

const REPO: &str = "SleepyMiles/miles-lc-mods";
const API: &str = "https://api.github.com";

/// The only GitHub account allowed to publish.
const ALLOWED_LOGIN: &str = "SleepyMiles";

/// Client ID of the "SleepyMiles Mod Manager" OAuth App on SleepyMiles' GitHub account
/// (Settings -> Developer settings -> OAuth Apps, with "Enable Device Flow" ticked).
/// Client IDs are public; device flow needs no client secret.
const GITHUB_CLIENT_ID: &str = "Ov23liAvteaTQHaquzIL";

#[derive(Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SiteModInfo {
    pub path: PathBuf,
    pub file_name: String,
    pub size: u64,
    pub name: String,
    pub version: String,
    pub description: String,
    pub website_url: String,
    pub dependencies: Vec<String>,
    /// `data:image/png;base64,...` when the zip has an icon.png.
    pub icon: Option<String>,
    /// Release tag and asset name this would be published as.
    pub tag: String,
    pub asset_name: String,
    /// Things that stop it from being published.
    pub problems: Vec<String>,
    /// Things worth knowing that don't block publishing.
    pub notes: Vec<String>,
    pub already_published: bool,
}

/// Reads and checks a mod zip without touching the network.
pub fn read_mod_zip(path: &Path) -> Result<SiteModInfo> {
    let file = std::fs::File::open(path).context("couldn't open the zip")?;
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut zip = zip::ZipArchive::new(file).context("this isn't a valid zip file")?;

    let mut info = SiteModInfo {
        path: path.to_path_buf(),
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size,
        ..Default::default()
    };

    let names: Vec<String> = zip.file_names().map(str::to_owned).collect();
    let find = |wanted: &str| names.iter().find(|n| n.eq_ignore_ascii_case(wanted)).cloned();

    let Some(manifest_name) = find("manifest.json") else {
        info.problems.push(
            "manifest.json must be at the top level of the zip (not inside a folder).".into(),
        );
        return Ok(info);
    };

    let mut raw = String::new();
    zip.by_name(&manifest_name)?
        .read_to_string(&mut raw)
        .context("manifest.json isn't valid text")?;
    let manifest: Value = match serde_json::from_str(raw.trim_start_matches('\u{feff}')) {
        Ok(v) => v,
        Err(err) => {
            info.problems
                .push(format!("manifest.json isn't valid JSON: {err}"));
            return Ok(info);
        }
    };

    check_manifest(&manifest, &mut info);

    if let Some(icon) = find("icon.png") {
        let mut bytes = Vec::new();
        if zip.by_name(&icon)?.read_to_end(&mut bytes).is_ok() {
            info.icon = Some(format!("data:image/png;base64,{}", BASE64_STANDARD.encode(bytes)));
        }
    } else {
        info.notes
            .push("No icon.png at the top of the zip, so the mod has no picture.".into());
    }
    if find("README.md").is_none() {
        info.notes
            .push("No README.md, so the mod page will be empty.".into());
    }

    if !info.name.is_empty() && !info.version.is_empty() {
        info.tag = format!("{}-{}", info.name, info.version);
        info.asset_name = format!("{}.zip", info.tag);
    }

    Ok(info)
}

/// The same rules the site's publish workflow uses (and that Gale needs to import the zip).
fn check_manifest(manifest: &Value, info: &mut SiteModInfo) {
    let text = |key: &str| manifest.get(key).and_then(Value::as_str);

    match text("name") {
        Some(name)
            if !name.is_empty()
                && name.len() <= 100
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') =>
        {
            info.name = name.to_owned()
        }
        Some(name) => info.problems.push(format!(
            "The name \"{name}\" may only use letters, numbers and _ (like Thunderstore)."
        )),
        None => info.problems.push("manifest.json needs a \"name\".".into()),
    }

    match text("version_number") {
        Some(v) if is_plain_version(v) => info.version = v.to_owned(),
        Some(v) => info.problems.push(format!(
            "The version \"{v}\" must look like 1.2.3."
        )),
        None => info
            .problems
            .push("manifest.json needs a \"version_number\".".into()),
    }

    match text("description") {
        Some(d) => info.description = d.to_owned(),
        None => info
            .problems
            .push("manifest.json needs a \"description\" (it can be empty).".into()),
    }

    match text("website_url") {
        Some(w) => info.website_url = w.to_owned(),
        None => info.problems.push(
            "manifest.json needs a \"website_url\" (it can be empty: \"\"), or Gale won't import it."
                .into(),
        ),
    }

    match manifest.get("dependencies").and_then(Value::as_array) {
        Some(deps) => {
            for dep in deps {
                match dep.as_str().map(str::trim) {
                    Some(d) if is_dependency(d) => info.dependencies.push(d.to_owned()),
                    _ => info.problems.push(format!(
                        "The dependency {dep} must look like Author-ModName-1.2.3."
                    )),
                }
            }
        }
        None => info.problems.push(
            "manifest.json needs a \"dependencies\" list (it can be empty: []).".into(),
        ),
    }
}

fn is_plain_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn is_dependency(dep: &str) -> bool {
    let parts: Vec<&str> = dep.split('-').collect();
    parts.len() == 3
        && parts[..2]
            .iter()
            .all(|p| !p.is_empty() && !p.chars().any(char::is_whitespace))
        && is_plain_version(parts[2])
}

fn github_token() -> Result<String> {
    token::get(Backend::SleepyMiles)?
        .ok_or_else(|| eyre!("you're not logged in to GitHub yet"))
}

// ---------------------------------------------------------------- GitHub login (device flow)

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLogin {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum LoginPoll {
    /// Waiting for the code to be entered on github.com. Poll again after `interval` seconds.
    Pending { interval: u64 },
    LoggedIn { login: String },
}

fn client_id() -> Result<&'static str> {
    if GITHUB_CLIENT_ID.starts_with("REPLACE_") {
        bail!("GitHub login isn't set up in this build yet (missing OAuth client ID)");
    }
    Ok(GITHUB_CLIENT_ID)
}

#[command]
pub async fn site_login_start(app: AppHandle) -> crate::util::cmd::Result<DeviceLogin> {
    let response = app
        .http()
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .json(&json!({ "client_id": client_id()?, "scope": "public_repo" }))
        .send()
        .await
        .map_err(|err| eyre!("couldn't reach GitHub: {err}"))?;
    let v: Value = response.json().await.map_err(|err| eyre!("{err}"))?;
    if let Some(err) = v.get("error_description").or(v.get("error")).and_then(Value::as_str) {
        return Err(eyre!("GitHub login failed: {err}").into());
    }
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let n = |k: &str, d: u64| v.get(k).and_then(Value::as_u64).unwrap_or(d);
    Ok(DeviceLogin {
        device_code: s("device_code"),
        user_code: s("user_code"),
        verification_uri: s("verification_uri"),
        interval: n("interval", 5),
        expires_in: n("expires_in", 900),
    })
}

/// One poll of GitHub's token endpoint. Saves the token only if it belongs to SleepyMiles.
#[command]
pub async fn site_login_poll(
    device_code: String,
    interval: u64,
    app: AppHandle,
) -> crate::util::cmd::Result<LoginPoll> {
    let response = app
        .http()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .json(&json!({
            "client_id": client_id()?,
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        }))
        .send()
        .await
        .map_err(|err| eyre!("couldn't reach GitHub: {err}"))?;
    let v: Value = response.json().await.map_err(|err| eyre!("{err}"))?;

    if let Some(token) = v.get("access_token").and_then(Value::as_str) {
        let login = account_login(&app, token).await?;
        if !login.eq_ignore_ascii_case(ALLOWED_LOGIN) {
            return Err(eyre!(
                "only {ALLOWED_LOGIN} can upload to the site (you logged in as {login})"
            )
            .into());
        }
        token::set(Backend::SleepyMiles, token)?;
        info!("logged in to GitHub as {login}");
        return Ok(LoginPoll::LoggedIn { login });
    }

    match v.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => Ok(LoginPoll::Pending { interval }),
        Some("slow_down") => Ok(LoginPoll::Pending {
            interval: v.get("interval").and_then(Value::as_u64).unwrap_or(interval + 5),
        }),
        Some("expired_token") => Err(eyre!("the login code expired; start again").into()),
        Some("access_denied") => Err(eyre!("the login was cancelled on GitHub").into()),
        Some(other) => Err(eyre!(
            "GitHub login failed: {}",
            v.get("error_description").and_then(Value::as_str).unwrap_or(other)
        )
        .into()),
        None => Err(eyre!("GitHub sent an unexpected answer").into()),
    }
}

async fn account_login(app: &AppHandle, token: &str) -> Result<String> {
    let response = github(app, reqwest::Method::GET, &format!("{API}/user"), token)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(explain(response, "read your GitHub account").await);
    }
    let v: Value = response.json().await?;
    v.get("login")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| eyre!("GitHub didn't say which account this is"))
}

/// The logged-in account, or None when logged out or the saved login no longer works.
#[command]
pub async fn site_account(app: AppHandle) -> crate::util::cmd::Result<Option<String>> {
    let Some(token) = token::get(Backend::SleepyMiles)? else {
        return Ok(None);
    };
    match account_login(&app, &token).await {
        Ok(login) if login.eq_ignore_ascii_case(ALLOWED_LOGIN) => Ok(Some(login)),
        Ok(_) | Err(_) => Ok(None),
    }
}

#[command]
pub fn site_logout() -> crate::util::cmd::Result<()> {
    token::clear(Backend::SleepyMiles)?;
    Ok(())
}

fn github(
    app: &AppHandle,
    method: reqwest::Method,
    url: &str,
    token: &str,
) -> reqwest_middleware::RequestBuilder {
    app.http()
        .request(method, url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
}

/// Turns GitHub's error responses into something actionable.
async fn explain(response: reqwest::Response, action: &str) -> eyre::Report {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let detail = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or(body);
    match status.as_u16() {
        401 => eyre!("your GitHub login expired while trying to {action}. Log in again."),
        403 | 404 => eyre!(
            "your GitHub login isn't allowed to {action} on {REPO}. Log out and log in again. (GitHub said: {detail})"
        ),
        _ => eyre!("couldn't {action}: GitHub answered {status}: {detail}"),
    }
}

async fn release_exists(app: &AppHandle, token: &str, tag: &str) -> Result<bool> {
    let url = format!("{API}/repos/{REPO}/releases/tags/{tag}");
    let response = github(app, reqwest::Method::GET, &url, token).send().await?;
    match response.status().as_u16() {
        200 => Ok(true),
        404 => Ok(false),
        _ => Err(explain(response, "check existing releases").await),
    }
}

#[command]
pub async fn inspect_site_mod(path: PathBuf, app: AppHandle) -> crate::util::cmd::Result<SiteModInfo> {
    let mut info = read_mod_zip(&path)?;
    if !info.tag.is_empty() {
        if let Ok(token) = github_token() {
            match release_exists(&app, &token, &info.tag).await {
                Ok(exists) => info.already_published = exists,
                Err(err) => warn!("couldn't check if {} exists: {err:#}", info.tag),
            }
        }
    }
    if info.already_published {
        info.problems.push(format!(
            "{} {} is already on the site. Raise version_number in manifest.json to publish an update.",
            info.name, info.version
        ));
    }
    Ok(info)
}

#[command]
pub async fn publish_site_mod(path: PathBuf, app: AppHandle) -> crate::util::cmd::Result<String> {
    Ok(publish(&path, &app).await?)
}

async fn publish(path: &Path, app: &AppHandle) -> Result<String> {
    let token = github_token()?;
    let login = account_login(app, &token).await?;
    if !login.eq_ignore_ascii_case(ALLOWED_LOGIN) {
        bail!("only {ALLOWED_LOGIN} can upload to the site (logged in as {login})");
    }
    let info = read_mod_zip(path)?;
    if let Some(problem) = info.problems.first() {
        bail!("{problem}");
    }
    if release_exists(app, &token, &info.tag).await? {
        bail!(
            "{} {} is already on the site. Raise version_number in manifest.json to publish an update.",
            info.name,
            info.version
        );
    }

    info!("publishing {} to {REPO}", info.asset_name);

    // 1. Draft release, so the site's workflow doesn't start before the zip is attached.
    let body = format!(
        "{}\n\nPublished from SleepyMiles Mod Manager.",
        if info.description.is_empty() { &info.name } else { &info.description }
    );
    let response = github(app, reqwest::Method::POST, &format!("{API}/repos/{REPO}/releases"), &token)
        .json(&json!({
            "tag_name": info.tag,
            "target_commitish": "main",
            "name": format!("{} {}", info.name, info.version),
            "body": body,
            "draft": true,
        }))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(explain(response, "create a release").await);
    }
    let release: Value = response.json().await?;
    let id = release["id"]
        .as_u64()
        .ok_or_else(|| eyre!("GitHub didn't return a release id"))?;

    let result = attach_and_publish(app, &token, id, &release, &info).await;
    if result.is_err() {
        // Don't leave a half-made draft behind.
        let url = format!("{API}/repos/{REPO}/releases/{id}");
        if let Err(err) = github(app, reqwest::Method::DELETE, &url, &token).send().await {
            warn!("couldn't delete draft release {id}: {err:#}");
        }
    }
    result
}

async fn attach_and_publish(
    app: &AppHandle,
    token: &str,
    id: u64,
    release: &Value,
    info: &SiteModInfo,
) -> Result<String> {
    // 2. Upload the zip under the exact name the site and the app download.
    let upload_base = release["upload_url"]
        .as_str()
        .and_then(|u| u.split('{').next())
        .ok_or_else(|| eyre!("GitHub didn't return an upload address"))?;
    let bytes = tokio::fs::read(&info.path)
        .await
        .context("couldn't read the zip")?;
    let response = github(app, reqwest::Method::POST, &format!("{upload_base}?name={}", info.asset_name), token)
        .header("Content-Type", "application/zip")
        .body(bytes)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(explain(response, "upload the zip").await);
    }

    // 3. Publish. This is what starts the site's "Publish to mod site" workflow.
    let response = github(
        app,
        reqwest::Method::PATCH,
        &format!("{API}/repos/{REPO}/releases/{id}"),
        token,
    )
    .json(&json!({ "draft": false, "make_latest": "false" }))
    .send()
    .await?;
    if !response.status().is_success() {
        return Err(explain(response, "publish the release").await);
    }
    let published: Value = response.json().await?;
    let url = published["html_url"].as_str().unwrap_or_default().to_owned();
    info!("published {} at {url}", info.asset_name);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (n, data) in files {
            zip.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn good_zip_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = br#"{"name":"BunkbedRevive_LegitsFork","version_number":"2.0.4","description":"Revive","website_url":"","dependencies":["BepInEx-BepInExPack-5.4.2304","AinaVT-LethalConfig-1.4.6"]}"#;
        let zip = make_zip(dir.path(), "x.zip", &[("manifest.json", manifest), ("icon.png", b"png"), ("README.md", b"hi")]);
        let info = read_mod_zip(&zip).unwrap();
        assert!(info.problems.is_empty(), "{:?}", info.problems);
        assert_eq!(info.tag, "BunkbedRevive_LegitsFork-2.0.4");
        assert_eq!(info.asset_name, "BunkbedRevive_LegitsFork-2.0.4.zip");
        assert_eq!(info.dependencies.len(), 2);
        assert!(info.icon.unwrap().starts_with("data:image/png;base64,"));
        assert!(info.notes.is_empty());
    }

    #[test]
    fn bom_and_missing_icon_are_handled() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = "\u{feff}{\"name\":\"A_b1\",\"version_number\":\"1.0.0\",\"description\":\"\",\"website_url\":\"\",\"dependencies\":[]}";
        let zip = make_zip(dir.path(), "x.zip", &[("manifest.json", manifest.as_bytes())]);
        let info = read_mod_zip(&zip).unwrap();
        assert!(info.problems.is_empty(), "{:?}", info.problems);
        assert_eq!(info.notes.len(), 2);
    }

    #[test]
    fn bad_manifests_are_explained() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = br#"{"name":"LC Office Patcher","version_number":"1.0","dependencies":["NotADep"]}"#;
        let zip = make_zip(dir.path(), "x.zip", &[("manifest.json", manifest)]);
        let info = read_mod_zip(&zip).unwrap();
        let all = info.problems.join("\n");
        assert!(all.contains("may only use letters"), "{all}");
        assert!(all.contains("must look like 1.2.3"), "{all}");
        assert!(all.contains("description"), "{all}");
        assert!(all.contains("website_url"), "{all}");
        assert!(all.contains("Author-ModName-1.2.3"), "{all}");
        assert!(info.tag.is_empty());

        let zip = make_zip(dir.path(), "y.zip", &[("Folder/manifest.json", b"{}")]);
        assert!(read_mod_zip(&zip).unwrap().problems[0].contains("top level"));
    }

    #[test]
    fn version_and_dependency_rules() {
        assert!(is_plain_version("1.2.3") && !is_plain_version("1.2") && !is_plain_version("1.2.3-beta"));
        assert!(is_dependency("Rune580-LethalCompany_InputUtils-0.7.13"));
        assert!(!is_dependency("Rune580-Lethal-Company-0.7.13") && !is_dependency("Rune580-InputUtils"));
    }
}

#[cfg(test)]
mod real_zip_check {
    /// Set SITE_ZIPS to a ':'-separated list of zips to check real mods (skipped otherwise).
    #[test]
    fn real_zips_pass() {
        let Ok(list) = std::env::var("SITE_ZIPS") else { return };
        for p in list.split(':') {
            let info = super::read_mod_zip(std::path::Path::new(p)).unwrap();
            println!("{} -> tag {} problems {:?} notes {:?}", p, info.tag, info.problems, info.notes);
            assert!(info.problems.is_empty());
        }
    }
}
