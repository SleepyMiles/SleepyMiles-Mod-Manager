//! "Send logs to Miles": zips the game's logs and posts them to Miles's Discord channel.
//!
//! The zip has the active profile's BepInEx log, Lethal Company's Player.log (and Player-prev.log,
//! which has the crash when the game was restarted after it), this app's own log, and a list of the
//! profile's mods. The Discord webhook comes from `app-config.json` on Miles's site (stored base64'd
//! so scanners don't flag it), so he can reset it without an app update.

use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::Engine;
use eyre::{bail, ensure, eyre, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{command, AppHandle};

use crate::{profile::ModManager, state::ManagerExt, thunderstore::Backend};

const CONFIG_URL: &str = "https://sleepymiles.github.io/miles-lc-mods/app-config.json";
/// Logs bigger than this keep their start and end, with the middle cut out.
const MAX_LOG: u64 = 24 * 1024 * 1024;
const KEEP_HEAD: u64 = 3 * 1024 * 1024;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    /// Name inside the zip.
    name: String,
    label: String,
    path: String,
    exists: bool,
    size: u64,
    /// Unix seconds.
    modified: Option<u64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogsInfo {
    profile: String,
    mod_count: usize,
    files: Vec<LogFile>,
}

fn player_log_dir() -> Option<PathBuf> {
    dirs_next::home_dir().map(|h| {
        h.join("AppData")
            .join("LocalLow")
            .join("ZeekerssRBLX")
            .join("Lethal Company")
    })
}

fn log_file(name: &str, label: &str, path: PathBuf) -> LogFile {
    let meta = fs::metadata(&path).ok();
    LogFile {
        name: name.to_owned(),
        label: label.to_owned(),
        exists: meta.as_ref().is_some_and(|m| m.is_file()),
        size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        modified: meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs()),
        path: path.display().to_string(),
    }
}

fn collect(manager: &ModManager) -> (LogsInfo, String) {
    let profile = manager.active_profile();
    let mut files = Vec::new();
    if let Ok(path) = profile.log_path() {
        files.push(log_file("LogOutput.log", "BepInEx log (this profile)", path));
    }
    if let Some(dir) = player_log_dir() {
        files.push(log_file("Player.log", "Lethal Company Player.log", dir.join("Player.log")));
        files.push(log_file(
            "Player-prev.log",
            "Previous Player.log (from before the last restart)",
            dir.join("Player-prev.log"),
        ));
    }
    files.push(log_file("MilesModManager.log", "Miles Mod Manager log", crate::logger::log_path()));

    let mut mods = String::new();
    for m in &profile.mods {
        let ident = m.ident();
        let source = match m.backend() {
            Backend::SleepyMiles => " (Miles's site)",
            _ => "",
        };
        let kind = if m.kind.as_thunderstore().is_some() { "" } else { " (local)" };
        mods.push_str(&format!(
            "[{}] {} {}{source}{kind}\n",
            if m.enabled { "on " } else { "off" },
            ident.full_name(),
            ident.version(),
        ));
    }

    (
        LogsInfo {
            profile: profile.name.clone(),
            mod_count: profile.mods.len(),
            files,
        },
        mods,
    )
}

#[command]
pub fn logs_info(app: AppHandle) -> crate::util::cmd::Result<LogsInfo> {
    Ok(collect(&app.lock_manager()).0)
}

/// Reads a log even while the game has it open, cutting the middle out of huge ones.
fn read_log(path: &Path) -> Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len <= MAX_LOG {
        let mut buf = Vec::with_capacity(len as usize);
        file.read_to_end(&mut buf)?;
        return Ok(buf);
    }
    let tail = MAX_LOG - KEEP_HEAD;
    let mut head = vec![0; KEEP_HEAD as usize];
    file.read_exact(&mut head)?;
    file.seek(SeekFrom::Start(len - tail))?;
    let mut end = Vec::with_capacity(tail as usize);
    file.read_to_end(&mut end)?;
    head.extend_from_slice(
        format!(
            "\n\n===== {} MB cut out of the middle of this log by Miles Mod Manager =====\n\n",
            (len - MAX_LOG) / 1024 / 1024
        )
        .as_bytes(),
    );
    head.extend(end);
    Ok(head)
}

fn build_zip(info: &LogsInfo, mods: &str, note: &str) -> Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(9));
        let mut missing = Vec::new();
        for f in &info.files {
            if !f.exists {
                missing.push(f.name.as_str());
                continue;
            }
            match read_log(Path::new(&f.path)) {
                Ok(data) => {
                    zip.start_file(&f.name, options)?;
                    zip.write_all(&data)?;
                }
                Err(err) => {
                    tracing::warn!("couldn't read {}: {err}", f.path);
                    missing.push(f.name.as_str());
                }
            }
        }
        zip.start_file("mods.txt", options)?;
        zip.write_all(format!("Profile: {} ({} mods)\n\n{mods}", info.profile, info.mod_count).as_bytes())?;
        let mut readme = format!("{note}\n");
        if !missing.is_empty() {
            readme.push_str(&format!("\nNot found: {}\n", missing.join(", ")));
        }
        zip.start_file("info.txt", options)?;
        zip.write_all(readme.as_bytes())?;
        zip.finish()?;
    }
    Ok(buf.into_inner())
}

fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(32)
        .collect();
    if s.trim_matches('_').is_empty() { "friend".into() } else { s }
}

fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string()
}

fn note(app: &AppHandle, name: &str, message: &str, profile: &str) -> String {
    format!(
        "From: {name}\nProfile: {profile}\nApp: Miles Mod Manager {}\nSent: {}\n\n{message}",
        app.package_info().version,
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
    )
}

async fn webhook_url(app: &AppHandle) -> Result<String> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let config: Value = app
        .http()
        .get(format!("{CONFIG_URL}?t={secs}"))
        .send()
        .await
        .context("couldn't reach Miles's site")?
        .error_for_status()
        .map_err(|_| eyre!("Miles hasn't set up log sending yet. Use \"Save as zip\" and send the file instead."))?
        .json()
        .await
        .context("Miles's site sent a broken settings file")?;
    let encoded = config
        .get("logWebhook")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| eyre!("Log sending is turned off right now. Use \"Save as zip\" and send the file instead."))?;
    let url = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(encoded.trim())?)?;
    ensure!(
        url.starts_with("https://discord.com/api/webhooks/")
            || url.starts_with("https://discordapp.com/api/webhooks/"),
        "Miles's site has a bad log address. Use \"Save as zip\" instead."
    );
    Ok(url)
}

#[command]
pub async fn send_logs(name: String, message: String, app: AppHandle) -> crate::util::cmd::Result<()> {
    Ok(send(name, message, app).await?)
}

async fn send(name: String, message: String, app: AppHandle) -> Result<()> {
    let name = name.trim().chars().take(40).collect::<String>();
    ensure!(!name.is_empty(), "type your name first, so Miles knows who sent it.");
    let message = message.trim().chars().take(1500).collect::<String>();

    let (info, mods) = collect(&app.lock_manager());
    ensure!(
        info.files.iter().any(|f| f.exists),
        "no log files found. Launch the game with this profile once, then try again."
    );
    let note_text = note(&app, &name, &message, &info.profile);
    let zip = tauri::async_runtime::spawn_blocking({
        let info = info.clone();
        move || build_zip(&info, &mods, &note_text)
    })
    .await
    .map_err(|e| eyre!(e))??;

    let url = webhook_url(&app).await?;

    let mut content = format!(
        "📋 **Logs from {}**\nProfile: **{}** ({} mods) · app {}",
        name.replace(['*', '_', '`', '~', '|', '>'], ""),
        info.profile,
        info.mod_count,
        app.package_info().version
    );
    if !message.is_empty() {
        for line in message.lines().take(15) {
            content.push_str("\n> ");
            content.push_str(line);
        }
    }
    let content: String = content.chars().take(1990).collect();

    let payload = json!({
        "username": "Miles Mod Manager",
        "content": content,
        "allowed_mentions": { "parse": [] },
    });
    let file_name = format!("logs_{}_{}.zip", safe_name(&name), stamp());
    let form = reqwest::multipart::Form::new()
        .text("payload_json", payload.to_string())
        .part(
            "files[0]",
            reqwest::multipart::Part::bytes(zip)
                .file_name(file_name)
                .mime_str("application/zip")?,
        );

    let response = reqwest::Client::new()
        .post(format!("{url}?wait=true"))
        .multipart(form)
        .send()
        .await
        .context("couldn't reach Discord")?;

    match response.status().as_u16() {
        200..=299 => Ok(()),
        413 => bail!("the logs are too big for Discord. Use \"Save as zip\" instead."),
        429 => bail!("too many sends in a row. Wait a minute and try again."),
        401 | 403 | 404 => bail!(
            "Miles's log channel isn't accepting logs (the webhook was removed). Use \"Save as zip\" and send the file instead."
        ),
        code => {
            let body = response.text().await.unwrap_or_default();
            bail!("Discord said {code}: {}", body.chars().take(200).collect::<String>())
        }
    }
}

/// Saves the same zip to the Downloads folder and shows it in Explorer. Returns its path.
#[command]
pub async fn save_logs(name: String, message: String, app: AppHandle) -> crate::util::cmd::Result<String> {
    Ok(save(name, message, app).await?)
}

async fn save(name: String, message: String, app: AppHandle) -> Result<String> {
    let (info, mods) = collect(&app.lock_manager());
    ensure!(
        info.files.iter().any(|f| f.exists),
        "no log files found. Launch the game with this profile once, then try again."
    );
    let who = if name.trim().is_empty() { "friend".to_owned() } else { name.trim().to_owned() };
    let note_text = note(&app, &who, message.trim(), &info.profile);
    let dir = dirs_next::download_dir()
        .or_else(|| dirs_next::home_dir().map(|h| h.join("Downloads")).filter(|d| d.is_dir()))
        .or_else(dirs_next::desktop_dir)
        .or_else(dirs_next::home_dir)
        .ok_or_else(|| eyre!("couldn't find your Downloads folder"))?;
    let path = dir.join(format!("Miles-logs_{}_{}.zip", safe_name(&who), stamp()));
    let out = path.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        let zip = build_zip(&info, &mods, &note_text)?;
        fs::write(&out, zip).context("couldn't save the zip")?;
        Ok(())
    })
    .await
    .map_err(|e| eyre!(e))??;

    reveal(&path);
    Ok(path.display().to_string())
}

#[cfg(windows)]
fn reveal(path: &Path) {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
}

#[cfg(not(windows))]
fn reveal(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = open::that_detached(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_logs_keep_start_and_end() {
        let dir = std::env::temp_dir().join(format!("mmm-logs-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.log");
        let mut data = vec![b'a'; (MAX_LOG + 5 * 1024 * 1024) as usize];
        data[0] = b'S';
        *data.last_mut().unwrap() = b'E';
        fs::write(&path, &data).unwrap();
        let out = read_log(&path).unwrap();
        assert!(out.len() < (MAX_LOG + 1024) as usize);
        assert_eq!(out[0], b'S');
        assert_eq!(*out.last().unwrap(), b'E');
        assert!(String::from_utf8_lossy(&out).contains("cut out of the middle"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn zip_has_logs_and_mod_list() {
        let dir = std::env::temp_dir().join(format!("mmm-zip-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("LogOutput.log"), "[Info] hello").unwrap();
        let info = LogsInfo {
            profile: "tt".into(),
            mod_count: 1,
            files: vec![
                log_file("LogOutput.log", "x", dir.join("LogOutput.log")),
                log_file("Player.log", "y", dir.join("missing.log")),
            ],
        };
        let bytes = build_zip(&info, "[on ] A-B 1.0.0\n", "From: Bob").unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let names: Vec<_> = zip.file_names().map(str::to_owned).collect();
        assert!(names.contains(&"LogOutput.log".to_owned()));
        assert!(names.contains(&"mods.txt".to_owned()));
        let mut readme = String::new();
        zip.by_name("info.txt").unwrap().read_to_string(&mut readme).unwrap();
        assert!(readme.contains("Not found: Player.log"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_are_safe_in_file_names() {
        assert_eq!(safe_name("Bob <3"), "Bob__3");
        assert_eq!(safe_name("  "), "friend");
    }
}
