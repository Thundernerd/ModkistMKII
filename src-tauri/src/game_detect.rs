use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::game_path::{validate_directory, GAME_EXECUTABLE, STEAM_APP_ID};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePathCandidate {
    pub path: String,
    pub source: String,
}

pub fn detect_game_paths() -> Vec<GamePathCandidate> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    for steamapps in steamapps_directories() {
        if let Some(path) = game_path_from_steamapps(&steamapps) {
            push_candidate(&mut candidates, &mut seen, path, "Steam");
        }
    }

    for root in prefix_scan_roots() {
        for path in find_zeepkist_dirs_in_tree(&root, 12) {
            push_candidate(&mut candidates, &mut seen, path, "Wine prefix");
        }
    }

    candidates
}

fn push_candidate(
    candidates: &mut Vec<GamePathCandidate>,
    seen: &mut HashSet<String>,
    path: PathBuf,
    source: &str,
) {
    if validate_directory(&path).is_err() {
        return;
    }

    let key = normalize_path_key(&path);
    if !seen.insert(key) {
        return;
    }

    candidates.push(GamePathCandidate {
        path: path.to_string_lossy().into_owned(),
        source: source.into(),
    });
}

fn normalize_path_key(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn steamapps_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    for root in steam_install_roots() {
        let libraryfolders = root.join("config/libraryfolders.vdf");
        if libraryfolders.is_file() {
            if let Ok(content) = fs::read_to_string(libraryfolders) {
                for library_path in paths_from_libraryfolders_vdf(&content) {
                    let steamapps = PathBuf::from(library_path).join("steamapps");
                    let key = steamapps.to_string_lossy().into_owned();
                    if seen.insert(key) {
                        dirs.push(steamapps);
                    }
                }
            }
        }

        let default_steamapps = root.join("steamapps");
        let key = default_steamapps.to_string_lossy().into_owned();
        if seen.insert(key) {
            dirs.push(default_steamapps);
        }
    }

    dirs
}

fn steam_install_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    #[cfg(windows)]
    {
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            roots.push(PathBuf::from(program_files_x86).join("Steam"));
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            roots.push(PathBuf::from(program_files).join("Steam"));
        }
    }

    if let Some(home) = home_dir() {
        #[cfg(target_os = "linux")]
        {
            roots.push(home.join(".local/share/Steam"));
            roots.push(home.join(".steam/steam"));
            roots.push(home.join(".steam/debian-installation"));
        }

        #[cfg(target_os = "macos")]
        {
            roots.push(home.join("Library/Application Support/Steam"));
        }
    }

    roots
}

fn paths_from_libraryfolders_vdf(content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"path\"") {
            if let Some(path) = vdf_value_after_key(trimmed, "\"path\"") {
                paths.push(path);
            }
        }
    }
    paths
}

fn vdf_value_after_key(line: &str, key: &str) -> Option<String> {
    let index = line.find(key)?;
    let after_key = &line[index + key.len()..];
    let value_start = after_key.find('"')?;
    parse_vdf_quoted_value(&after_key[value_start..]).map(unescape_vdf_path)
}

fn unescape_vdf_path(value: String) -> String {
    value.replace("\\\\", "\\")
}

fn game_path_from_steamapps(steamapps: &Path) -> Option<PathBuf> {
    let manifest = steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf"));
    if !manifest.is_file() {
        return None;
    }

    let content = fs::read_to_string(manifest).ok()?;
    let installdir = installdir_from_manifest(&content)?;
    let game_dir = steamapps.join("common").join(installdir);
    if game_dir.join(GAME_EXECUTABLE).is_file() {
        Some(game_dir)
    } else {
        None
    }
}

fn installdir_from_manifest(content: &str) -> Option<String> {
    vdf_value_after_key(content, "\"installdir\"")
}

fn parse_vdf_quoted_value(value: &str) -> Option<String> {
    let rest = value.trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn prefix_scan_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let Some(home) = home_dir() else {
        return roots;
    };

    #[cfg(target_os = "macos")]
    {
        roots.push(home.join("Library/Application Support/CrossOver/Bottles"));
        roots.push(home.join("Library/Application Support/CrossOver Games/Bottles"));
        if let Ok(output) = Command::new("defaults")
            .args(["read", "com.codeweavers.CrossOver", "BottleDir"])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    roots.insert(0, PathBuf::from(path));
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        roots.push(home.join(".local/share/Steam/steamapps/compatdata"));
        roots.push(home.join(".steam/debian-installation/steamapps/compatdata"));
        roots.push(home.join(".steam/steam/steamapps/compatdata"));
        roots.push(home.join("Games/Heroic/Prefixes"));
        roots.push(home.join(".wine"));
        roots.push(home.join(".cxoffice"));
    }

    roots
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn find_zeepkist_dirs_in_tree(root: &Path, max_depth: u32) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if !root.is_dir() {
        return found;
    }
    walk_for_zeepkist(root, max_depth, &mut found);
    found
}

fn walk_for_zeepkist(dir: &Path, depth: u32, found: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if path.file_name().and_then(|name| name.to_str()) == Some(GAME_EXECUTABLE) {
                if let Some(parent) = path.parent() {
                    found.push(parent.to_path_buf());
                }
            }
        } else if path.is_dir() {
            walk_for_zeepkist(&path, depth - 1, found);
        }
    }
}

#[tauri::command]
pub fn detect_game_paths_command() -> Vec<GamePathCandidate> {
    let candidates = detect_game_paths();
    log::info!(
        "Detected {} Zeepkist install candidate(s)",
        candidates.len()
    );
    for candidate in &candidates {
        log::debug!(
            "Detected Zeepkist candidate via {} at {}",
            candidate.source,
            candidate.path
        );
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_installdir_from_manifest() {
        let content = "\"AppState\"\n{\n\t\"installdir\"\t\t\"Zeepkist\"\n}\n";
        assert_eq!(
            installdir_from_manifest(content).as_deref(),
            Some("Zeepkist")
        );
    }

    #[test]
    fn finds_game_from_steamapps_layout() {
        let root = std::env::temp_dir().join("modkist-detect-steam");
        let steamapps = root.join("steamapps");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(steamapps.join("common/Zeepkist")).unwrap();
        fs::write(steamapps.join("common/Zeepkist/zeepkist.exe"), b"").unwrap();
        fs::write(
            steamapps.join("appmanifest_1440670.acf"),
            "\"AppState\"\n{\n\t\"installdir\"\t\t\"Zeepkist\"\n}\n",
        )
        .unwrap();

        let game = game_path_from_steamapps(&steamapps).expect("game path");
        assert!(game.ends_with("common/Zeepkist"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn parses_libraryfolders_paths() {
        let content = r#"
"libraryfolders"
{
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}
"#;
        assert_eq!(
            paths_from_libraryfolders_vdf(content),
            vec!["D:\\SteamLibrary".to_string()]
        );
    }
}
