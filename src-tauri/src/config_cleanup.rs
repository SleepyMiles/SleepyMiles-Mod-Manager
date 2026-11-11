//! "Remove unused config files": finds config files and sections left behind by mods that are no
//! longer in the profile, without touching anything a mod still uses.
//!
//! Two kinds of leftovers are found:
//!
//! 1. **Whole files.** BepInEx writes `## Plugin GUID: <guid>` at the top of every config file it
//!    makes for a plugin. That GUID is a constant inside the plugin's DLL (in its `[BepInPlugin]`
//!    attribute), so if no DLL in the profile contains the GUID (disabled mods included), the
//!    plugin is gone and so is the need for its file. Files without that header are never touched.
//!
//! 2. **Sections in LethalLevelLoader.cfg.** LethalLevelLoader writes one section per dungeon and
//!    moon ("Custom Dungeon:  Toy Store"). It only binds them when a lobby loads
//!    (`StartOfRound.Awake`), but rewrites the file as soon as the game starts, so after a session
//!    that only reached the main menu *every* dungeon looks unused, live ones too. BepInEx writes a
//!    bound setting with its `# Setting type:` comment and an unbound ("orphaned") one without it.
//!    So a custom dungeon or moon section is only called unused when the same file shows that the
//!    lobby binding ran in the last session (some Vanilla/Custom section is bound) and this
//!    section still has no bound setting.
//!
//! Before changing anything the whole config folder is zipped into the app's data folder.

use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use aho_corasick::AhoCorasick;
use eyre::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle};
use tracing::info;

use crate::state::ManagerExt;

const LLL_FILE: &str = "LethalLevelLoader.cfg";
const LLL_BINDING_SECTIONS: &[&str] = &["Vanilla Dungeon:", "Vanilla Level:", "Custom Dungeon:", "Custom Level:"];
const LLL_CONTENT_SECTIONS: &[&str] = &["Custom Dungeon:", "Custom Level:"];
const KEEP_BACKUPS: usize = 5;

// ---------------------------------------------------------------- reading cfg files

#[derive(Debug, Clone, PartialEq)]
struct SectionInfo {
    /// Exactly as in the file, between the brackets (LethalLevelLoader pads names with spaces).
    raw_name: String,
    bound: usize,
    settings: usize,
}

impl SectionInfo {
    /// Without the padding. LethalLevelLoader sorts its sections by putting zero-width spaces
    /// (U+200B) in front of the names, which `trim()` doesn't count as whitespace.
    fn name(&self) -> &str {
        self.raw_name.trim_matches(|c: char| c.is_whitespace() || c == '\u{200b}' || c == '\u{feff}')
    }
}

#[derive(Debug, Clone)]
struct CfgInfo {
    guid: Option<String>,
    plugin: Option<String>,
    sections: Vec<SectionInfo>,
}

fn section_header(line: &str) -> Option<&str> {
    let t = line.trim_end_matches(['\r', '\n']).trim_end();
    let t = t.trim_start_matches('\u{feff}');
    t.strip_prefix('[')?.strip_suffix(']')
}

fn parse_cfg(text: &str) -> CfgInfo {
    let mut info = CfgInfo {
        guid: None,
        plugin: None,
        sections: Vec::new(),
    };
    for line in text.lines() {
        let l = line.trim_start_matches('\u{feff}').trim();
        if info.sections.is_empty() {
            if let Some(g) = l.strip_prefix("## Plugin GUID:") {
                info.guid = Some(g.trim().to_owned());
                continue;
            }
            if let Some(p) = l.strip_prefix("## Settings file was created by plugin") {
                info.plugin = Some(p.trim().to_owned());
                continue;
            }
        }
        if let Some(name) = section_header(line) {
            info.sections.push(SectionInfo {
                raw_name: name.to_owned(),
                bound: 0,
                settings: 0,
            });
            continue;
        }
        let Some(section) = info.sections.last_mut() else { continue };
        if l.starts_with("# Setting type:") {
            section.bound += 1;
        } else if !l.is_empty() && !l.starts_with('#') && l.contains('=') {
            section.settings += 1;
        }
    }
    info
}

/// Removes whole sections (header to the next header), keeping every other byte as it was.
fn remove_sections(text: &str, raw_names: &HashSet<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for line in text.split_inclusive('\n') {
        if let Some(name) = section_header(line) {
            skipping = raw_names.contains(name);
        }
        if !skipping {
            out.push_str(line);
        }
    }
    out
}

// ---------------------------------------------------------------- finding plugins

/// Which of `guids` appear in any DLL in the profile (disabled `.old` copies included).
fn guids_in_dlls(profile: &Path, guids: &[String]) -> Result<HashSet<usize>> {
    let mut patterns: Vec<Vec<u8>> = Vec::new();
    let mut owner = Vec::new();
    for (i, g) in guids.iter().enumerate() {
        patterns.push(g.as_bytes().to_vec());
        owner.push(i);
        patterns.push(g.encode_utf16().flat_map(u16::to_le_bytes).collect());
        owner.push(i);
    }
    let ac = AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build(&patterns)
        .context("couldn't build the search")?;

    let mut found = HashSet::new();
    let mut dlls = 0;
    for entry in walkdir::WalkDir::new(profile).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !(name.ends_with(".dll") || name.ends_with(".dll.old")) {
            continue;
        }
        dlls += 1;
        let Ok(data) = fs::read(entry.path()) else { continue };
        for m in ac.find_overlapping_iter(&data) {
            found.insert(owner[m.pattern().as_usize()]);
        }
        if found.len() == guids.len() {
            break;
        }
    }
    ensure!(dlls > 0, "this profile has no mod files at all, so nothing can be checked");
    Ok(found)
}

// ---------------------------------------------------------------- scan

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct UnusedFile {
    /// Relative to the profile, with forward slashes.
    path: String,
    plugin: String,
    size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct UnusedSection {
    path: String,
    /// Trimmed, for display and for matching.
    section: String,
    settings: usize,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanupScan {
    profile: String,
    files: Vec<UnusedFile>,
    sections: Vec<UnusedSection>,
    /// Things that couldn't be checked yet, in plain words.
    notes: Vec<String>,
    checked_files: usize,
}

fn rel(profile: &Path, path: &Path) -> String {
    path.strip_prefix(profile)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn scan(profile: &Path) -> Result<CleanupScan> {
    let config_dir = profile.join("BepInEx").join("config");
    let mut out = CleanupScan::default();
    if !config_dir.is_dir() {
        return Ok(out);
    }

    let mut cfgs: Vec<(PathBuf, String, CfgInfo)> = Vec::new();
    for entry in walkdir::WalkDir::new(&config_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("cfg")) {
            continue;
        }
        let Ok(bytes) = fs::read(path) else { continue };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let info = parse_cfg(&text);
        cfgs.push((path.to_owned(), text, info));
    }
    out.checked_files = cfgs.len();

    // 1. whole files whose plugin is gone
    let guids: Vec<String> = cfgs.iter().filter_map(|(_, _, i)| i.guid.clone()).collect::<HashSet<_>>().into_iter().collect();
    if !guids.is_empty() {
        let present = guids_in_dlls(profile, &guids)?;
        let present: HashSet<&str> = present.iter().map(|&i| guids[i].as_str()).collect();
        for (path, _, info) in &cfgs {
            let Some(guid) = &info.guid else { continue };
            if !present.contains(guid.as_str()) {
                out.files.push(UnusedFile {
                    path: rel(profile, path),
                    plugin: info.plugin.clone().unwrap_or_else(|| guid.clone()),
                    size: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
                });
            }
        }
    }
    let dead_files: HashSet<&str> = out.files.iter().map(|f| f.path.as_str()).collect();

    // 2. LethalLevelLoader's dungeons and moons
    let mut live_content: HashSet<String> = HashSet::new();
    let mut dead_content: HashSet<String> = HashSet::new();
    for (path, _, info) in &cfgs {
        let r = rel(profile, path);
        if dead_files.contains(r.as_str()) || !path.file_name().is_some_and(|n| n.eq_ignore_ascii_case(LLL_FILE)) {
            continue;
        }
        let starts = |s: &SectionInfo, list: &[&str]| list.iter().any(|p| s.name().starts_with(p));
        let binding_ran = info.sections.iter().any(|s| starts(s, LLL_BINDING_SECTIONS) && s.bound > 0);
        let candidates: Vec<&SectionInfo> = info
            .sections
            .iter()
            .filter(|s| starts(s, LLL_CONTENT_SECTIONS) && s.bound == 0 && s.settings > 0)
            .collect();
        if candidates.is_empty() {
            continue;
        }
        if binding_ran {
            for s in &info.sections {
                if starts(s, LLL_BINDING_SECTIONS) {
                    let name = content_name(s.name());
                    if s.bound > 0 {
                        live_content.insert(name);
                    } else {
                        dead_content.insert(name);
                    }
                }
            }
            out.sections.extend(candidates.into_iter().map(|s| UnusedSection {
                path: r.clone(),
                section: s.name().to_owned(),
                settings: s.settings,
            }));
        } else {
            out.notes.push(
                "LethalLevelLoader's dungeons and moons can't be checked yet: the last time you played this profile, \
                 the game didn't get as far as a lobby. Host or join a lobby once, quit, then check again."
                    .to_owned(),
            );
        }
    }

    // 3. JLL makes one small file per interior/moon ("BepInEx/config/JLL/Magic Wesley.Toy Store.cfg")
    //    without a plugin header. It's only called unused when LethalLevelLoader (checked above,
    //    after a lobby session) shows that same interior or moon as removed and nothing live has
    //    that name.
    for (path, _, info) in &cfgs {
        if info.guid.is_some() {
            continue;
        }
        let in_jll = path
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|n| n.eq_ignore_ascii_case("JLL"));
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
        let Some((author, content)) = stem.split_once('.') else { continue };
        let key = content.trim().to_lowercase();
        if in_jll && dead_content.contains(&key) && !live_content.contains(&key) {
            out.files.push(UnusedFile {
                path: rel(profile, path),
                plugin: format!("JLL settings for {} ({})", content.trim(), author.trim()),
                size: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            });
        }
    }

    out.files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// "Custom Dungeon:  Toy Store" -> "toy store", "Custom Level:  240 Alcatras" -> "alcatras".
fn content_name(section: &str) -> String {
    let rest = section.split_once(':').map(|(_, r)| r).unwrap_or(section).trim();
    let rest = match rest.split_once(' ') {
        Some((num, name)) if section.contains("Level:") && num.chars().all(|c| c.is_ascii_digit()) => name,
        _ => rest,
    };
    rest.trim().to_lowercase()
}

fn active_profile(app: &AppHandle) -> (String, PathBuf) {
    let manager = app.lock_manager();
    let p = manager.active_profile();
    (p.name.clone(), p.path.clone())
}

#[command]
pub async fn scan_unused_config(app: AppHandle) -> crate::util::cmd::Result<CleanupScan> {
    let (name, path) = active_profile(&app);
    let mut result = tauri::async_runtime::spawn_blocking(move || scan(&path))
        .await
        .map_err(|e| eyre::eyre!(e))??;
    result.profile = name;
    Ok(result)
}

// ---------------------------------------------------------------- apply

fn backup(app: &AppHandle, profile_name: &str, config_dir: &Path) -> Result<PathBuf> {
    let dir = app.lock_prefs().data_dir.join("config-backups");
    fs::create_dir_all(&dir)?;
    let safe: String = profile_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let path = dir.join(format!("{safe}_{}.zip", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")));

    let file = fs::File::create(&path).context("couldn't create the backup")?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in walkdir::WalkDir::new(config_dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = rel(config_dir, entry.path());
        zip.start_file(name, options)?;
        zip.write_all(&fs::read(entry.path())?)?;
    }
    zip.finish()?;

    // keep only the newest few backups of this profile
    let prefix = format!("{safe}_");
    let mut old: Vec<PathBuf> = fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix)))
        .collect();
    old.sort();
    while old.len() > KEEP_BACKUPS {
        fs::remove_file(old.remove(0)).ok();
    }
    Ok(path)
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    files_removed: usize,
    sections_removed: usize,
    backup: String,
}

/// Removes the chosen leftovers. Everything is checked again first, and only items that are
/// still unused are removed, so a stale list from the screen can't delete anything in use.
#[command]
pub async fn remove_unused_config(
    files: Vec<String>,
    sections: Vec<UnusedSection>,
    app: AppHandle,
) -> crate::util::cmd::Result<CleanupResult> {
    let (name, profile) = active_profile(&app);
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<CleanupResult> {
        ensure!(
            !app2.install_queue().lock().is_processing(),
            "mods are being installed right now. Wait until that's done, then try again."
        );
        let fresh = scan(&profile)?;
        let files: Vec<&UnusedFile> = fresh.files.iter().filter(|f| files.contains(&f.path)).collect();
        let sections: Vec<&UnusedSection> = fresh.sections.iter().filter(|s| sections.contains(s)).collect();
        if files.is_empty() && sections.is_empty() {
            bail!("nothing to remove (it may have changed since you checked). Check again.");
        }

        let config_dir = profile.join("BepInEx").join("config");
        let backup = backup(&app2, &name, &config_dir)?;

        for f in &files {
            let path = profile.join(&f.path);
            ensure!(path.starts_with(&config_dir), "refusing to delete outside the config folder");
            fs::remove_file(&path).with_context(|| format!("couldn't delete {}", f.path))?;
        }

        let mut by_file: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
        for s in &sections {
            by_file.entry(s.path.as_str()).or_default().push(s.section.as_str());
        }
        for (file, names) in by_file {
            let path = profile.join(file);
            let text = String::from_utf8_lossy(&fs::read(&path)?).into_owned();
            let info = parse_cfg(&text);
            let raw: HashSet<String> = info
                .sections
                .iter()
                .filter(|s| names.contains(&s.name()))
                .map(|s| s.raw_name.clone())
                .collect();
            let new_text = remove_sections(&text, &raw);
            fs::write(&path, new_text).with_context(|| format!("couldn't save {file}"))?;
        }

        info!(files = files.len(), sections = sections.len(), backup = %backup.display(), "removed unused config");
        Ok(CleanupResult {
            files_removed: files.len(),
            sections_removed: sections.len(),
            backup: backup.display().to_string(),
        })
    })
    .await
    .map_err(|e| eyre::eyre!(e))??;

    // the config editor reads files from a cache; make it notice
    app.lock_manager().active_profile_mut().refresh_config();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LLL: &str = "## Settings file was created by plugin LethalLevelLoader v1.4.0\r\n\
## Plugin GUID: imabatby.lethallevelloader\r\n\
\r\n\
[\u{200b}\u{200b}\u{200b}\u{200b}\u{200b} - LethalLevelLoader Settings -]\r\n\
\r\n\
## Controls debug\r\n\
# Setting type: DebugType\r\n\
# Default value: User\r\n\
LethalLevelLoader Debugging Mode = User\r\n\
\r\n\
[\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}Vanilla Dungeon:  Facility (Level1Flow)]\r\n\
\r\n\
# Setting type: Boolean\r\n\
# Default value: false\r\n\
General Settings - Enable Content Configuration = false\r\n\
\r\n\
[\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}Custom Dungeon:  Office]\r\n\
\r\n\
# Setting type: Boolean\r\n\
# Default value: false\r\n\
General Settings - Enable Content Configuration = true\r\n\
\r\n\
[\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}Custom Dungeon:  Toy Store]\r\n\
\r\n\
General Settings - Enable Content Configuration = true\r\n\
Dungeon Injection Settings - Manual Level Names List = Rend:300\r\n\
\r\n";

    #[test]
    fn parses_bound_and_orphaned() {
        let info = parse_cfg(LLL);
        assert_eq!(info.guid.as_deref(), Some("imabatby.lethallevelloader"));
        let toy = info.sections.iter().find(|s| s.name() == "Custom Dungeon:  Toy Store").unwrap();
        assert_eq!((toy.bound, toy.settings), (0, 2));
        let office = info.sections.iter().find(|s| s.name() == "Custom Dungeon:  Office").unwrap();
        assert_eq!((office.bound, office.settings), (1, 1));
    }

    #[test]
    fn removes_only_the_dead_section() {
        let info = parse_cfg(LLL);
        let raw: HashSet<String> = info
            .sections
            .iter()
            .filter(|s| s.name() == "Custom Dungeon:  Toy Store")
            .map(|s| s.raw_name.clone())
            .collect();
        let out = remove_sections(LLL, &raw);
        assert!(!out.contains("Toy Store"));
        assert!(!out.contains("Rend:300"));
        assert!(out.contains("Custom Dungeon:  Office]\r\n"));
        assert_eq!(out, LLL.split("[\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}\u{200b}Custom Dungeon:  Toy Store]").next().unwrap());
    }

    fn profile_with(name: &str, lll: &str, dll_text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mmm-cfg-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("BepInEx/config")).unwrap();
        fs::create_dir_all(dir.join("BepInEx/plugins/LLL")).unwrap();
        fs::write(dir.join("BepInEx/config").join(LLL_FILE), lll).unwrap();
        fs::write(
            dir.join("BepInEx/config/gone.mod.cfg"),
            "## Settings file was created by plugin Gone v1\n## Plugin GUID: gone.mod\n\n[A]\n\n# Setting type: Boolean\nX = true\n",
        )
        .unwrap();
        fs::write(
            dir.join("BepInEx/config/kept.disabled.cfg"),
            "## Settings file was created by plugin Kept v1\n## Plugin GUID: kept.disabled\n\n[A]\nX = 1\n",
        )
        .unwrap();
        fs::write(dir.join("BepInEx/config/custom.cfg"), "[A]\nX = 1\n").unwrap();
        fs::write(dir.join("BepInEx/plugins/LLL/LethalLevelLoader.dll"), dll_text).unwrap();
        // a disabled mod: Gale renames its files to .old
        let utf16: Vec<u8> = "kept.disabled".encode_utf16().flat_map(u16::to_le_bytes).collect();
        fs::write(dir.join("BepInEx/plugins/Kept.dll.old"), utf16).unwrap();
        dir
    }

    #[test]
    fn finds_removed_plugins_and_dead_dungeons() {
        let dir = profile_with("a", LLL, "....IMABATBY.LethalLevelLoader....");
        let result = scan(&dir).unwrap();
        let files: Vec<_> = result.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(files, vec!["BepInEx/config/gone.mod.cfg"]);
        let sections: Vec<_> = result.sections.iter().map(|s| s.section.as_str()).collect();
        assert_eq!(sections, vec!["Custom Dungeon:  Toy Store"]);
        assert!(result.notes.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Miles's real LethalLevelLoader.cfg (after a session that only reached the main menu),
    /// if it's around: copied to /mnt/user-data/uploads/config/ for this test.
    #[test]
    fn real_file_after_main_menu_session() {
        let Ok(real) = fs::read_to_string("/mnt/user-data/uploads/config/LethalLevelLoader.cfg") else { return };
        let dir = profile_with("real", &real, "x");
        let result = scan(&dir).unwrap();
        assert!(result.sections.is_empty(), "nothing may be removed after a menu-only session");
        assert_eq!(result.notes.len(), 1);

        // pretend a lobby loaded: everything bound except Toy Store and Alcatras
        let mut lobby = String::new();
        let mut current = String::new();
        for line in real.split_inclusive('\n') {
            if let Some(h) = section_header(line) {
                current = h.to_owned();
            }
            let t = line.trim_start();
            let dead = current.contains("Toy Store") || current.contains("Alcatras");
            if !dead && !t.starts_with('#') && !t.starts_with('[') && t.contains('=') {
                lobby.push_str("# Setting type: Boolean\r\n");
            }
            lobby.push_str(line);
        }
        let dir2 = profile_with("real2", &lobby, "x");
        fs::create_dir_all(dir2.join("BepInEx/config/JLL")).unwrap();
        for f in ["Magic Wesley.Toy Store.cfg", "MagicWesley.Alcatras.cfg", "MagicWesley.Polarus.cfg", "Magic Wesley.Hallowed content.cfg"] {
            fs::write(dir2.join("BepInEx/config/JLL").join(f), "[A]\nX = 1\n").unwrap();
        }
        let result = scan(&dir2).unwrap();
        let mut sections: Vec<_> = result.sections.iter().map(|s| s.section.as_str()).collect();
        sections.sort();
        assert_eq!(sections, vec!["Custom Dungeon:  Toy Store", "Custom Level:  240 Alcatras"]);
        let files: Vec<_> = result.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            files,
            vec![
                "BepInEx/config/JLL/Magic Wesley.Toy Store.cfg",
                "BepInEx/config/JLL/MagicWesley.Alcatras.cfg",
                "BepInEx/config/gone.mod.cfg"
            ]
        );
        // LC Office and everything else stays
        assert!(!result.sections.iter().any(|s| s.section.contains("LC Office")));
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(&dir2).unwrap();
    }

    #[test]
    fn content_names() {
        assert_eq!(content_name("Custom Dungeon:  Toy Store"), "toy store");
        assert_eq!(content_name("Custom Level:  240 Alcatras"), "alcatras");
        assert_eq!(content_name("Custom Level:  End"), "end");
        assert_eq!(content_name("Vanilla Dungeon:  Facility (Level1Flow)"), "facility (level1flow)");
    }

    #[test]
    fn main_menu_only_session_removes_no_dungeons() {
        // after a session that never reached a lobby nothing but the general settings are bound
        let menu_only = LLL
            .replace("# Setting type: Boolean\r\n# Default value: false\r\n", "")
            .to_string();
        let dir = profile_with("b", &menu_only, "imabatby.lethallevelloader");
        let result = scan(&dir).unwrap();
        assert!(result.sections.is_empty());
        assert_eq!(result.notes.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
