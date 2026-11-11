//! "Free up space": works out how much disk space mod files really use and removes what nothing needs.
//!
//! Mods are installed by hard-linking files from the download cache into each profile, so a file that
//! is in the cache *and* in a profile only takes space once. Deleting the cached copy of an installed
//! mod frees nothing (the profile still has it), and deleting a profile frees nothing for mods whose
//! cached copy is kept. All sizes shown to the user are therefore "bytes that would really be freed",
//! counted per physical file.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use eyre::{ensure, Context, Result};
use serde::Serialize;
use tauri::{command, AppHandle};
use tracing::{info, warn};

use crate::{
    profile::{ManagedGame, ModManager},
    state::ManagerExt,
};

// ---------------------------------------------------------------- physical file sizes

/// (volume, file index, number of hard links) for a file.
#[cfg(windows)]
fn file_id(path: &Path) -> Option<(u64, u64, u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION},
    };

    let file = fs::File::open(path).ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }.ok()?;
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    Some((u64::from(info.dwVolumeSerialNumber), index, info.nNumberOfLinks))
}

#[cfg(unix)]
fn file_id(path: &Path) -> Option<(u64, u64, u32)> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::symlink_metadata(path).ok()?;
    Some((meta.dev(), meta.ino(), meta.nlink() as u32))
}

/// How many bytes deleting everything under `roots` would actually free. A hard-linked file only
/// counts if every one of its links is inside `roots`.
pub fn freeable_size<P: AsRef<Path>>(roots: &[P]) -> u64 {
    struct Seen {
        links: u32,
        found: u32,
        size: u64,
    }

    let mut seen: HashMap<(u64, u64), Seen> = HashMap::new();
    let mut unknown = 0;

    for root in roots {
        for entry in walkdir::WalkDir::new(root.as_ref())
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            match file_id(entry.path()) {
                Some((vol, index, links)) => {
                    let e = seen.entry((vol, index)).or_insert(Seen {
                        links,
                        found: 0,
                        size,
                    });
                    e.found += 1;
                }
                // couldn't open it (locked); assume it's a single copy
                None => unknown += size,
            }
        }
    }

    unknown
        + seen
            .values()
            .filter(|s| s.found >= s.links.max(1))
            .map(|s| s.size)
            .sum::<u64>()
}

pub fn total_size<P: AsRef<Path>>(roots: &[P]) -> u64 {
    roots
        .iter()
        .flat_map(|root| walkdir::WalkDir::new(root.as_ref()).follow_links(false))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

fn remove_all<P: AsRef<Path>>(paths: &[P]) {
    for path in paths {
        let path = path.as_ref();
        let result = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        if let Err(err) = result {
            warn!("failed to delete {}: {err}", path.display());
        }
    }
}

/// Removes now-empty package folders in the cache.
fn tidy_cache(cache: &Path) {
    let Ok(entries) = fs::read_dir(cache) else { return };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() && path.read_dir().is_ok_and(|mut d| d.next().is_none()) {
            fs::remove_dir(&path).ok();
        }
    }
}

// ---------------------------------------------------------------- what's unused

/// (full name, version) of every Thunderstore/site mod in any profile, optionally leaving one profile out.
fn used_mods(manager: &ModManager, except_profile: Option<i64>) -> HashSet<(String, String)> {
    manager
        .games
        .values()
        .flat_map(|game| game.profiles.iter())
        .filter(|profile| Some(profile.id) != except_profile)
        .flat_map(|profile| profile.thunderstore_mods())
        .map(|(m, _)| (m.ident.full_name().to_owned(), m.ident.version().to_owned()))
        .collect()
}

/// Cached mod versions that no profile uses.
fn unused_cache_dirs(cache: &Path, used: &HashSet<(String, String)>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(packages) = fs::read_dir(cache) else { return out };
    for package in packages.filter_map(Result::ok) {
        let package_path = package.path();
        if !package_path.is_dir() {
            continue;
        }
        let name = package.file_name().to_string_lossy().into_owned();
        let Ok(versions) = fs::read_dir(&package_path) else { continue };
        for version in versions.filter_map(Result::ok) {
            if !version.path().is_dir() {
                continue;
            }
            let ver = version.file_name().to_string_lossy().into_owned();
            if !used.contains(&(name.clone(), ver)) {
                out.push(version.path());
            }
        }
    }
    out
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let s = p.to_string_lossy().replace('\\', "/");
        let s = s.trim_end_matches('/').to_owned();
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    norm(a) == norm(b)
}

/// Folders in a game's profiles folder that don't belong to any profile (left behind by crashes or
/// failed deletes).
fn leftover_profile_dirs(game: &ManagedGame) -> Vec<PathBuf> {
    let root = game.path.join("profiles");
    let Ok(entries) = fs::read_dir(&root) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| !game.profiles.iter().any(|profile| same_path(&profile.path, p)))
        .collect()
}

struct Plan {
    cache: PathBuf,
    unused_cache: Vec<PathBuf>,
    leftover_profiles: Vec<PathBuf>,
}

fn plan(app: &AppHandle) -> Plan {
    let cache = app.lock_prefs().cache_dir();
    let manager = app.lock_manager();
    let used = used_mods(&manager, None);
    Plan {
        unused_cache: unused_cache_dirs(&cache, &used),
        leftover_profiles: manager.games.values().flat_map(leftover_profile_dirs).collect(),
        cache,
    }
}

fn ensure_not_installing(app: &AppHandle) -> Result<()> {
    ensure!(
        !app.install_queue().lock().is_processing(),
        "mods are being installed right now. Wait until that's done, then try again."
    );
    Ok(())
}

// ---------------------------------------------------------------- Gale leftovers

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GaleLeftovers {
    /// Bytes that deleting them would free.
    size: u64,
    paths: Vec<String>,
    /// Gale is still installed, so its profiles are still in use.
    installed: bool,
    running: bool,
}

const GALE_ID: &str = "com.kesomannen.gale";

/// Everything Gale keeps on disk. Gale's own folders are deleted whole. If Gale's data was moved to
/// a custom folder, only the parts Gale made there (its cache and game folders) are listed, never
/// the custom folder itself.
fn gale_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let Some(roaming) = dirs_next::data_dir().map(|d| d.join(GALE_ID)) else { return paths };

    if let Some(custom) = gale_custom_data_dir(&roaming) {
        if !same_path(&custom, &roaming) && custom.is_dir() {
            let mut parts = vec![custom.join("cache")];
            parts.extend(gale_game_slugs(&roaming).into_iter().map(|slug| custom.join(slug)));
            paths.extend(parts.into_iter().filter(|p| p.exists()));
        }
    }
    if roaming.exists() {
        paths.push(roaming);
    }
    if let Some(local) = dirs_next::data_local_dir().map(|d| d.join(GALE_ID)) {
        if local.exists() && !paths.iter().any(|p| same_path(p, &local)) {
            paths.push(local);
        }
    }
    paths
}

fn gale_db(roaming: &Path) -> Option<rusqlite::Connection> {
    let path = roaming.join("data.sqlite3");
    if !path.exists() {
        return None;
    }
    rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
}

fn gale_custom_data_dir(roaming: &Path) -> Option<PathBuf> {
    let db = gale_db(roaming)?;
    let json: String = db
        .query_row("SELECT data FROM prefs WHERE id = 1", [], |row| row.get(0))
        .ok()?;
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    value.get("dataDir")?.as_str().map(PathBuf::from)
}

fn gale_game_slugs(roaming: &Path) -> Vec<String> {
    let Some(db) = gale_db(roaming) else { return Vec::new() };
    let Ok(mut stmt) = db.prepare("SELECT slug FROM managed_games") else { return Vec::new() };
    let rows = stmt.query_map([], |row| row.get::<_, String>(0));
    rows.map(|r| r.filter_map(Result::ok).filter(|s| !s.is_empty() && !s.contains(['/', '\\', '.'])).collect())
        .unwrap_or_default()
}

#[cfg(windows)]
fn gale_installed() -> bool {
    use winreg::{enums::*, RegKey};
    let key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Gale";
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        if let Ok(k) = RegKey::predef(hive).open_subkey(key) {
            let location: String = k.get_value("InstallLocation").unwrap_or_default();
            let location = location.trim_matches('"');
            if location.is_empty() || Path::new(location).join("Gale.exe").exists() || Path::new(location).join("gale.exe").exists() {
                return true;
            }
        }
    }
    dirs_next::data_local_dir().is_some_and(|d| d.join("Gale").join("Gale.exe").exists())
}

#[cfg(not(windows))]
fn gale_installed() -> bool {
    false
}

#[cfg(windows)]
fn gale_running() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Gale.exe", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("gale.exe"))
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn gale_running() -> bool {
    false
}

fn gale_leftovers() -> Option<GaleLeftovers> {
    let paths = gale_paths();
    if paths.is_empty() {
        return None;
    }
    Some(GaleLeftovers {
        size: freeable_size(&paths),
        paths: paths.iter().map(|p| p.display().to_string()).collect(),
        installed: gale_installed(),
        running: gale_running(),
    })
}

// ---------------------------------------------------------------- commands

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StorageReport {
    /// What "Free up space" would free: unused cached versions + leftover profile folders.
    unused_size: u64,
    unused_mod_count: usize,
    leftover_profile_count: usize,
    /// Everything in the download cache, and how much of it clearing would really free.
    cache_size: u64,
    cache_freeable: u64,
    gale: Option<GaleLeftovers>,
}

#[command]
pub async fn storage_report(app: AppHandle) -> crate::util::cmd::Result<StorageReport> {
    let plan = plan(&app);
    let report = tauri::async_runtime::spawn_blocking(move || {
        let mut unused = plan.unused_cache.clone();
        unused.extend(plan.leftover_profiles.iter().cloned());
        StorageReport {
            unused_size: freeable_size(&unused),
            unused_mod_count: plan.unused_cache.len(),
            leftover_profile_count: plan.leftover_profiles.len(),
            cache_size: total_size(&[&plan.cache]),
            cache_freeable: freeable_size(&[&plan.cache]),
            gale: gale_leftovers(),
        }
    })
    .await
    .context("storage check failed")?;
    Ok(report)
}

/// Deletes cached mod versions no profile uses and leftover profile folders. Returns bytes freed.
#[command]
pub async fn free_unused_space(app: AppHandle) -> crate::util::cmd::Result<u64> {
    ensure_not_installing(&app)?;
    let plan = plan(&app);
    let freed = tauri::async_runtime::spawn_blocking(move || {
        let mut paths = plan.unused_cache;
        paths.extend(plan.leftover_profiles);
        let freed = freeable_size(&paths);
        remove_all(&paths);
        tidy_cache(&plan.cache);
        info!(count = paths.len(), freed, "freed unused space");
        freed
    })
    .await
    .context("cleanup failed")?;
    Ok(freed)
}

/// Deletes the whole download cache. Returns bytes freed.
#[command]
pub async fn clear_all_cache(app: AppHandle) -> crate::util::cmd::Result<u64> {
    ensure_not_installing(&app)?;
    let cache = app.lock_prefs().cache_dir();
    let freed = tauri::async_runtime::spawn_blocking(move || -> Result<u64> {
        let freed = freeable_size(&[&cache]);
        if cache.exists() {
            fs::remove_dir_all(&cache).context("failed to delete the mod cache")?;
            fs::create_dir_all(&cache).context("failed to recreate the mod cache folder")?;
        }
        info!(freed, "cleared mod cache");
        Ok(freed)
    })
    .await
    .context("clearing the cache failed")??;
    Ok(freed)
}

/// Deletes Gale's files. Refuses while Gale is open, and needs `even_if_installed` when Gale is
/// still installed (its profiles would be gone).
#[command]
pub async fn delete_gale_leftovers(even_if_installed: bool) -> crate::util::cmd::Result<u64> {
    let freed = tauri::async_runtime::spawn_blocking(move || -> Result<u64> {
        ensure!(!gale_running(), "Gale is open. Close Gale first, then try again.");
        ensure!(
            even_if_installed || !gale_installed(),
            "Gale is still installed, so these are its profiles."
        );
        let paths = gale_paths();
        let freed = freeable_size(&paths);
        remove_all(&paths);
        info!(?paths, freed, "deleted Gale's files");
        Ok(freed)
    })
    .await
    .context("deleting Gale's files failed")??;
    Ok(freed)
}

/// Before a profile is deleted: its folder plus the cached versions of mods no other profile uses.
/// Returns (cache folders to delete afterwards, bytes the whole deletion will free).
pub fn profile_deletion_plan(app: &AppHandle, profile_id: i64) -> Result<(Vec<PathBuf>, PathBuf, u64)> {
    let cache = app.lock_prefs().cache_dir();
    let manager = app.lock_manager();
    let profile = manager
        .games
        .values()
        .flat_map(|g| g.profiles.iter())
        .find(|p| p.id == profile_id)
        .ok_or_else(|| eyre::eyre!("profile not found"))?;
    let others = used_mods(&manager, Some(profile_id));
    let dirs: Vec<PathBuf> = profile
        .thunderstore_mods()
        .map(|(m, _)| (m.ident.full_name().to_owned(), m.ident.version().to_owned()))
        .filter(|key| !others.contains(key))
        .map(|(name, version)| cache.join(name).join(version))
        .filter(|p| p.exists())
        .collect();
    let profile_path = profile.path.clone();
    drop(manager);

    let mut all = dirs.clone();
    all.push(profile_path);
    let freed = freeable_size(&all);
    Ok((dirs, cache, freed))
}

pub fn finish_profile_deletion(dirs: Vec<PathBuf>, cache: PathBuf) {
    remove_all(&dirs);
    tidy_cache(&cache);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_links_count_once_and_only_when_all_links_go() {
        let dir = std::env::temp_dir().join(format!("mmm-storage-{}", std::process::id()));
        let cache = dir.join("cache");
        let profile = dir.join("profile");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(&profile).unwrap();
        fs::write(cache.join("a.dll"), vec![0u8; 1000]).unwrap();
        fs::hard_link(cache.join("a.dll"), profile.join("a.dll")).unwrap();
        fs::write(profile.join("config.cfg"), vec![0u8; 10]).unwrap();

        // deleting only the cache frees nothing: the profile still has the file
        assert_eq!(freeable_size(&[&cache]), 0);
        // deleting only the profile frees just its own config
        assert_eq!(freeable_size(&[&profile]), 10);
        // deleting both frees the dll once
        assert_eq!(freeable_size(&[&cache, &profile]), 1010);
        assert_eq!(total_size(&[&cache, &profile]), 2010);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unused_versions() {
        let dir = std::env::temp_dir().join(format!("mmm-unused-{}", std::process::id()));
        for (p, v) in [("A-Mod", "1.0.0"), ("A-Mod", "1.1.0"), ("B-Other", "2.0.0")] {
            fs::create_dir_all(dir.join(p).join(v)).unwrap();
        }
        let used: HashSet<_> = [("A-Mod".to_owned(), "1.1.0".to_owned())].into();
        let mut got: Vec<_> = unused_cache_dirs(&dir, &used)
            .into_iter()
            .map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        got.sort();
        assert_eq!(got, vec!["A-Mod/1.0.0", "B-Other/2.0.0"]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
