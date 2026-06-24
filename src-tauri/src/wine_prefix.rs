use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

const GAME_EXECUTABLE: &str = "zeepkist.exe";
const DLL_OVERRIDES_SECTION: &str = "[Software\\Wine\\DllOverrides]";
const OVERRIDE_VALUE: &str = "native,builtin";
const WINHTTP_KEYS: [&str; 2] = ["winhttp", "*winhttp"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WineWinhttpStatus {
    pub state: String,
    pub message: Option<String>,
    pub prefix_label: Option<String>,
}

impl WineWinhttpStatus {
    fn not_found() -> Self {
        Self {
            state: "notFound".into(),
            message: Some(
                "Could not find a Wine prefix for your game directory. Configure winhttp \
                 manually in Wine Configuration (Libraries → winhttp → native, builtin)."
                    .into(),
            ),
            prefix_label: None,
        }
    }

    fn already_configured(label: Option<String>) -> Self {
        Self {
            state: "alreadyConfigured".into(),
            message: None,
            prefix_label: label,
        }
    }

    fn applied(label: Option<String>) -> Self {
        Self {
            state: "applied".into(),
            message: None,
            prefix_label: label,
        }
    }

    fn failed(message: String) -> Self {
        Self {
            state: "failed".into(),
            message: Some(message),
            prefix_label: None,
        }
    }
}

#[derive(Debug, Clone)]
struct WinePrefix {
    path: PathBuf,
    label: Option<String>,
}

pub fn configure_winhttp_override(game_dir: &Path) -> WineWinhttpStatus {
    let Some(prefix) = find_wine_prefix_for_game_path(game_dir) else {
        return WineWinhttpStatus::not_found();
    };

    let user_reg = prefix.path.join("user.reg");
    let content = match fs::read_to_string(&user_reg) {
        Ok(content) => content,
        Err(error) => {
            return WineWinhttpStatus::failed(format!(
                "Could not read {}: {error}",
                user_reg.display()
            ));
        }
    };

    if is_winhttp_configured(&content) {
        return WineWinhttpStatus::already_configured(prefix.label);
    }

    if let Err(error) = apply_via_wine_reg(&prefix.path) {
        eprintln!("wine reg override failed, falling back to user.reg edit: {error}");
        if let Err(error) = apply_via_user_reg(&user_reg, &content) {
            return WineWinhttpStatus::failed(error);
        }
    }

    WineWinhttpStatus::applied(prefix.label)
}

fn find_wine_prefix_for_game_path(game_dir: &Path) -> Option<WinePrefix> {
    let canonical = canonicalize_path(game_dir).ok()?;

    if let Some(prefix) = find_prefix_via_drive_c(&canonical) {
        return Some(prefix);
    }

    if let Some(prefix) = find_proton_prefix(&canonical) {
        return Some(prefix);
    }

    find_prefix_via_scan(&canonical)
}

fn canonicalize_path(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|error| format!("Could not resolve {}: {error}", path.display()))
}

fn find_prefix_via_drive_c(game_dir: &Path) -> Option<WinePrefix> {
    let mut current = game_dir;
    while let Some(parent) = current.parent() {
        if current.file_name().and_then(|name| name.to_str()) == Some("drive_c") {
            let prefix_path = parent.to_path_buf();
            if prefix_path.join("user.reg").is_file() {
                let label = prefix_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string);
                return Some(WinePrefix { path: prefix_path, label });
            }
        }
        current = parent;
    }
    None
}

fn find_proton_prefix(game_dir: &Path) -> Option<WinePrefix> {
    let folder_name = game_dir.file_name()?.to_str()?;
    let mut current = game_dir;

    while let Some(parent) = current.parent() {
        if current.file_name().and_then(|name| name.to_str()) == Some("common")
            && parent.file_name().and_then(|name| name.to_str()) == Some("steamapps")
        {
            let steamapps = parent;
            let app_id = find_steam_app_id(steamapps, folder_name)?;
            let prefix_path = steamapps.join("compatdata").join(&app_id).join("pfx");
            if prefix_path.join("user.reg").is_file() {
                return Some(WinePrefix {
                    path: prefix_path,
                    label: Some(format!("Steam app {app_id}")),
                });
            }
            return None;
        }
        current = parent;
    }

    None
}

fn find_steam_app_id(steamapps: &Path, installdir: &str) -> Option<String> {
    let entries = fs::read_dir(steamapps).ok()?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let Some(app_id) = name
            .strip_prefix("appmanifest_")
            .and_then(|value| value.strip_suffix(".acf"))
        else {
            continue;
        };
        let content = fs::read_to_string(entry.path()).ok()?;
        if manifest_matches_installdir(&content, installdir) {
            return Some(app_id.to_string());
        }
    }
    None
}

fn manifest_matches_installdir(content: &str, installdir: &str) -> bool {
    installdir_from_manifest(content)
        .map(|value| value.eq_ignore_ascii_case(installdir))
        .unwrap_or(false)
}

fn installdir_from_manifest(content: &str) -> Option<String> {
    let needle = "\"installdir\"";
    let index = content.find(needle)?;
    let after_key = &content[index + needle.len()..];
    let value_start = after_key.find('"')?;
    parse_quoted_string(&after_key[value_start..])
}

fn find_prefix_via_scan(game_dir: &Path) -> Option<WinePrefix> {
    let home = home_dir()?;
    let roots = prefix_scan_roots(&home);

    for root in roots {
        if let Some(prefix) = scan_root_for_game(&root, game_dir) {
            return Some(prefix);
        }
    }

    None
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn prefix_scan_roots(home: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();

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

fn scan_root_for_game(root: &Path, game_dir: &Path) -> Option<WinePrefix> {
    if !root.is_dir() {
        return None;
    }

    let target_exe = game_dir.join(GAME_EXECUTABLE);
    let target_exe = canonicalize_path(&target_exe).ok()?;

    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let candidate_root = entry.path();
        if let Some(prefix_path) = find_matching_exe_in_tree(&candidate_root, &target_exe, 8) {
            let label = candidate_root
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string);
            if prefix_path.join("user.reg").is_file() {
                return Some(WinePrefix {
                    path: prefix_path,
                    label,
                });
            }
        }
    }

    None
}

fn find_matching_exe_in_tree(root: &Path, target_exe: &Path, max_depth: usize) -> Option<PathBuf> {
    let mut stack = vec![(root.to_path_buf(), 0usize)];

    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }

            if path.file_name().and_then(|name| name.to_str()) != Some(GAME_EXECUTABLE) {
                continue;
            }

            let canonical_exe = canonicalize_path(&path).ok();
            if canonical_exe.as_deref() == Some(target_exe) {
                return find_prefix_from_exe_path(&path);
            }
        }
    }

    None
}

fn find_prefix_from_exe_path(exe_path: &Path) -> Option<PathBuf> {
    let mut current = exe_path.parent()?;
    while let Some(parent) = current.parent() {
        if current.file_name().and_then(|name| name.to_str()) == Some("drive_c") {
            let prefix = parent.to_path_buf();
            if prefix.join("user.reg").is_file() {
                return Some(prefix);
            }
        }
        if current.join("user.reg").is_file() {
            return Some(current.to_path_buf());
        }
        current = parent;
    }
    None
}

pub(crate) fn is_winhttp_configured(content: &str) -> bool {
    let expected = normalize_override_value(OVERRIDE_VALUE);
    WINHTTP_KEYS.iter().all(|key| {
        read_dll_override_value(content, key)
            .map(|value| normalize_override_value(&value) == expected)
            .unwrap_or(false)
    })
}

fn normalize_override_value(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn read_dll_override_value(content: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == DLL_OVERRIDES_SECTION;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(value) = parse_reg_value_line(trimmed, key) {
            return Some(value);
        }
    }
    None
}

fn parse_reg_value_line(line: &str, key: &str) -> Option<String> {
    let prefix = format!("\"{key}\"=");
    if !line.starts_with(&prefix) {
        return None;
    }
    parse_quoted_string(line.strip_prefix(&prefix)?)
}

fn parse_quoted_string(value: &str) -> Option<String> {
    let value = value.trim();
    if !value.starts_with('"') {
        return None;
    }
    let inner = &value[1..];
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

pub(crate) fn merge_winhttp_overrides(content: &str) -> String {
    if is_winhttp_configured(content) {
        return content.to_string();
    }

    if let Some(start) = content.find(DLL_OVERRIDES_SECTION) {
        let after_header = start + DLL_OVERRIDES_SECTION.len();
        let remainder = &content[after_header..];
        let section_end = remainder
            .find("\n[")
            .map(|index| after_header + index)
            .unwrap_or(content.len());

        let mut section = content[start..section_end].to_string();
        for key in WINHTTP_KEYS {
            upsert_reg_entry_in_section(&mut section, key, OVERRIDE_VALUE);
        }

        return format!(
            "{}{}{}",
            &content[..start],
            section,
            &content[section_end..]
        );
    }

    let mut merged = content.to_string();
    if !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged.push('\n');
    merged.push_str(DLL_OVERRIDES_SECTION);
    merged.push('\n');
    for key in WINHTTP_KEYS {
        merged.push_str(&format!("\"{key}\"=\"{OVERRIDE_VALUE}\"\n"));
    }
    merged
}

fn upsert_reg_entry_in_section(section: &mut String, key: &str, value: &str) {
    let entry = format!("\"{key}\"=\"{value}\"");
    if let Some(line_index) = section.lines().position(|line| {
        line.trim()
            .starts_with(&format!("\"{key}\"="))
    }) {
        let lines: Vec<&str> = section.lines().collect();
        let mut rebuilt = String::new();
        for (index, line) in lines.iter().enumerate() {
            if index == line_index {
                rebuilt.push_str(&entry);
            } else {
                rebuilt.push_str(line);
            }
            if index + 1 < lines.len() {
                rebuilt.push('\n');
            }
        }
        *section = rebuilt;
        return;
    }

    if !section.ends_with('\n') {
        section.push('\n');
    }
    section.push_str(&entry);
    section.push('\n');
}

fn apply_via_user_reg(user_reg: &Path, content: &str) -> Result<(), String> {
    let merged = merge_winhttp_overrides(content);
    if merged == content {
        return Ok(());
    }

    let backup = user_reg.with_extension("reg.bak");
    fs::copy(user_reg, &backup).map_err(|error| {
        format!(
            "Could not back up {} to {}: {error}",
            user_reg.display(),
            backup.display()
        )
    })?;

    fs::write(user_reg, merged).map_err(|error| {
        format!("Could not write {}: {error}", user_reg.display())
    })
}

fn apply_via_wine_reg(prefix: &Path) -> Result<(), String> {
    let wine = find_wine_binary().ok_or_else(|| "No wine binary found".to_string())?;

    for key in WINHTTP_KEYS {
        let output = Command::new(&wine)
            .env("WINEPREFIX", prefix)
            .args([
                "reg",
                "add",
                r"HKCU\Software\Wine\DllOverrides",
                "/v",
                key,
                "/t",
                "REG_SZ",
                "/d",
                OVERRIDE_VALUE,
                "/f",
            ])
            .output()
            .map_err(|error| format!("Could not run {}: {error}", wine.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "wine reg add for {key} failed: {stdout} {stderr}"
            ));
        }
    }

    Ok(())
}

fn find_wine_binary() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let crossover_wine = PathBuf::from(
            "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine",
        );
        if crossover_wine.is_file() {
            return Some(crossover_wine);
        }
    }

    if command_exists("wine") {
        return Some(PathBuf::from("wine"));
    }

    None
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_prefix_via_drive_c() {
        let root = std::env::temp_dir().join("modkist-wine-test-drivec");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(
            root.join("drive_c/Program Files/Steam/steamapps/common/Zeepkist"),
        )
        .unwrap();
        fs::write(root.join("user.reg"), "REGEDIT4\n").unwrap();

        let game = root.join("drive_c/Program Files/Steam/steamapps/common/Zeepkist");
        let prefix = find_prefix_via_drive_c(&game).expect("prefix");
        assert!(prefix.path.ends_with("modkist-wine-test-drivec"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_proton_prefix_from_steam_layout() {
        let root = std::env::temp_dir().join("modkist-wine-test-proton");
        let steamapps = root.join("steamapps");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(steamapps.join("common/Zeepkist")).unwrap();
        fs::create_dir_all(steamapps.join("compatdata/1234567/pfx")).unwrap();
        fs::write(
            steamapps.join("appmanifest_1234567.acf"),
            "\"AppState\"\n{\n\t\"installdir\"\t\t\"Zeepkist\"\n}\n",
        )
        .unwrap();
        fs::write(
            steamapps.join("compatdata/1234567/pfx/user.reg"),
            "REGEDIT4\n",
        )
        .unwrap();

        let game = steamapps.join("common/Zeepkist");
        let prefix = find_proton_prefix(&game).expect("prefix");
        assert!(prefix.path.ends_with("compatdata/1234567/pfx"));
        assert_eq!(prefix.label.as_deref(), Some("Steam app 1234567"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_existing_winhttp_override() {
        let content = r#"REGEDIT4

[Software\Wine\DllOverrides]
"winhttp"="native,builtin"
"*winhttp"="native, builtin"
"#;
        assert!(is_winhttp_configured(content));
    }

    #[test]
    fn merges_missing_winhttp_override_section() {
        let content = "REGEDIT4\n";
        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains(DLL_OVERRIDES_SECTION));
        assert!(merged.contains("\"winhttp\"=\"native,builtin\""));
        assert!(merged.contains("\"*winhttp\"=\"native,builtin\""));
        assert!(is_winhttp_configured(&merged));
    }

    #[test]
    fn merges_into_existing_dll_overrides_section() {
        let content = r#"REGEDIT4

[Software\Wine\DllOverrides]
"d3d11"="builtin"
"#;
        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains("\"d3d11\"=\"builtin\""));
        assert!(is_winhttp_configured(&merged));
    }

    #[test]
    fn manifest_matches_installdir_case_insensitive() {
        let content = "\"installdir\"\t\t\"zeepkist\"\n";
        assert!(manifest_matches_installdir(content, "Zeepkist"));
    }
}
