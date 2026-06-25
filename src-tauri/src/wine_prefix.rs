use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

const GAME_EXECUTABLE: &str = "zeepkist.exe";
// Wine user.reg section headers escape backslashes (written as \\ in the file).
const USER_REG_DLL_OVERRIDES_SECTION: &str = "[Software\\\\Wine\\\\DllOverrides]";
const OVERRIDE_VALUE: &str = "native,builtin";
const WINHTTP_KEY: &str = "winhttp";
const WINE_REG_WINHTTP_KEYS: [&str; 2] = ["winhttp", "*winhttp"];

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
        log::warn!("wine reg override failed, falling back to user.reg edit: {error}");
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
    let Some(span) = primary_dll_overrides_section_span(content) else {
        return false;
    };

    read_dll_override_value_in_span(content, span, WINHTTP_KEY)
        .map(|value| normalize_override_value(&value) == expected)
        .unwrap_or(false)
}

fn is_dll_overrides_section(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') {
        return false;
    }

    let after_open = &trimmed[1..];
    let Some(close_index) = after_open.find(']') else {
        return false;
    };

    let section_name = &after_open[..close_index];
    section_name.replace("\\\\", "\\").to_ascii_lowercase() == r"software\wine\dlloverrides"
}

fn is_timestamped_dll_overrides_section(line: &str) -> bool {
    if !is_dll_overrides_section(line) {
        return false;
    }

    let trimmed = line.trim();
    let after_close = trimmed.split(']').nth(1).unwrap_or("").trim();
    after_close
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
}

fn find_dll_overrides_section_spans(content: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut section_start: Option<usize> = None;
    let mut offset = 0usize;

    for line in content.split_inclusive('\n') {
        let line_content = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = line_content.trim();

        if trimmed.starts_with('[') {
            if let Some(start) = section_start {
                spans.push((start, offset));
                section_start = None;
            }
            if is_dll_overrides_section(trimmed) {
                section_start = Some(offset);
            }
        }

        offset += line.len();
    }

    if let Some(start) = section_start {
        spans.push((start, content.len()));
    }

    spans
}

fn primary_dll_overrides_section_span(content: &str) -> Option<(usize, usize)> {
    let spans = find_dll_overrides_section_spans(content);
    if spans.is_empty() {
        return None;
    }

    for (start, end) in &spans {
        let header = content[*start..*end]
            .lines()
            .next()
            .unwrap_or_default()
            .trim();
        if is_timestamped_dll_overrides_section(header) {
            return Some((*start, *end));
        }
    }

    Some(spans[0])
}

fn find_dll_overrides_section_span(content: &str) -> Option<(usize, usize)> {
    primary_dll_overrides_section_span(content)
}

fn read_dll_override_value_in_span(content: &str, span: (usize, usize), key: &str) -> Option<String> {
    for line in content[span.0..span.1].lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') || trimmed.starts_with('#') {
            continue;
        }
        if let Some(value) = parse_reg_value_line(trimmed, key) {
            return Some(value);
        }
    }
    None
}

fn normalize_override_value(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
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

fn remove_plain_duplicate_dll_overrides_sections(content: &str) -> String {
    let spans = find_dll_overrides_section_spans(content);
    let has_timestamped = spans.iter().any(|(start, _)| {
        let header = content[*start..]
            .lines()
            .next()
            .unwrap_or_default()
            .trim();
        is_timestamped_dll_overrides_section(header)
    });

    if !has_timestamped {
        return content.to_string();
    }

    let mut removed = String::new();
    let mut cursor = 0usize;
    for (start, end) in spans {
        let header = content[start..end]
            .lines()
            .next()
            .unwrap_or_default()
            .trim();
        if is_dll_overrides_section(header) && !is_timestamped_dll_overrides_section(header) {
            removed.push_str(&content[cursor..start]);
            cursor = end;
        }
    }
    removed.push_str(&content[cursor..]);
    removed
}

pub(crate) fn merge_winhttp_overrides(content: &str) -> String {
    let content = remove_plain_duplicate_dll_overrides_sections(content);

    if is_winhttp_configured(&content) {
        return content;
    }

    if let Some((start, end)) = find_dll_overrides_section_span(&content) {
        let mut section = content[start..end].to_string();
        upsert_reg_entry_in_section(&mut section, WINHTTP_KEY, OVERRIDE_VALUE);

        return format!("{}{}{}", &content[..start], section, &content[end..]);
    }

    let mut merged = content;
    if !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged.push('\n');
    merged.push_str(USER_REG_DLL_OVERRIDES_SECTION);
    merged.push('\n');
    merged.push_str(&format!("\"{WINHTTP_KEY}\"=\"{OVERRIDE_VALUE}\"\n"));
    merged
}

fn reg_entry_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('"') {
        return None;
    }
    parse_quoted_string(trimmed)
}

fn compare_reg_entry_keys(left: &str, right: &str) -> std::cmp::Ordering {
    left.to_ascii_lowercase()
        .cmp(&right.to_ascii_lowercase())
}

fn trailing_blank_line_count(section: &str) -> usize {
    section
        .lines()
        .rev()
        .take_while(|line| line.trim().is_empty())
        .count()
}

fn upsert_reg_entry_in_section(section: &mut String, key: &str, value: &str) {
    let trailing_blanks = trailing_blank_line_count(section);
    let entry = format!("\"{key}\"=\"{value}\"");
    let lines: Vec<String> = section.lines().map(str::to_string).collect();

    let mut prefix = Vec::new();
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut suffix = Vec::new();
    let mut seen_entry = false;

    for line in lines {
        if let Some(entry_key) = reg_entry_key(&line) {
            seen_entry = true;
            if entry_key != key {
                entries.push((entry_key, line));
            }
        } else if !seen_entry {
            prefix.push(line);
        } else {
            suffix.push(line);
        }
    }

    entries.push((key.to_string(), entry));
    entries.sort_by(|(left, _), (right, _)| compare_reg_entry_keys(left, right));

    let mut result = prefix;
    result.extend(entries.into_iter().map(|(_, line)| line));
    result.extend(suffix);

    while result.last().is_some_and(|line| line.trim().is_empty()) {
        result.pop();
    }

    *section = result.join("\n");
    for _ in 0..=trailing_blanks {
        section.push('\n');
    }
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

    for key in WINE_REG_WINHTTP_KEYS {
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

[Software\\Wine\\DllOverrides]
"winhttp"="native,builtin"
"#;
        assert!(is_winhttp_configured(content));
    }

    #[test]
    fn detects_existing_winhttp_override_with_single_backslashes() {
        let content = r#"REGEDIT4

[Software\Wine\DllOverrides]
"winhttp"="native, builtin"
"#;
        assert!(is_winhttp_configured(content));
    }

    #[test]
    fn merges_missing_winhttp_override_section() {
        let content = "REGEDIT4\n";
        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains(USER_REG_DLL_OVERRIDES_SECTION));
        assert!(merged.contains("\"winhttp\"=\"native,builtin\""));
        assert!(is_winhttp_configured(&merged));
    }

    #[test]
    fn ignores_winhttp_in_duplicate_plain_section_when_timestamped_section_exists() {
        let content = r#"REGEDIT4

[Software\\Wine\\DllOverrides] 1608830137
#time=1d6da1865a6084e
"d3d11"="builtin"

[Software\\Wine\\DllOverrides]
"winhttp"="native,builtin"
"#;
        assert!(!is_winhttp_configured(content));

        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains(
            "[Software\\\\Wine\\\\DllOverrides] 1608830137\n#time=1d6da1865a6084e\n\"d3d11\"=\"builtin\"\n\"winhttp\"=\"native,builtin\"\n",
        ));
        assert!(!merged.contains("[Software\\\\Wine\\\\DllOverrides]\n\"winhttp\""));
        assert_eq!(merged.matches("[Software\\\\Wine\\\\DllOverrides]").count(), 1);
    }

    #[test]
    fn inserts_winhttp_in_alphabetical_order_among_existing_overrides() {
        let content = "[Software\\\\Wine\\\\DllOverrides] 1608830137\n#time=1d6da1865a6084e\n\"atlthunk\"=\"builtin\"\n\"winmm\"=\"builtin\"\n";
        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains(
            "#time=1d6da1865a6084e\n\"atlthunk\"=\"builtin\"\n\"winhttp\"=\"native,builtin\"\n\"winmm\"=\"builtin\"\n",
        ));
    }

    #[test]
    fn preserves_trailing_blank_line_before_next_section() {
        let content = "[Software\\\\Wine\\\\DllOverrides] 1608830137\n#time=1d6da1865a6084e\n\"d3d11\"=\"builtin\"\n\n[Software\\\\Wine\\\\AppDefaults]\n";
        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains(
            "\"d3d11\"=\"builtin\"\n\"winhttp\"=\"native,builtin\"\n\n[Software\\\\Wine\\\\AppDefaults]",
        ));
    }

    #[test]
    fn appends_winhttp_directly_after_last_entry_without_blank_line() {
        let content = "[Software\\\\Wine\\\\DllOverrides] 1608830137\n#time=1d6da1865a6084e\n\"d3d11\"=\"builtin\"\n";
        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains(
            "#time=1d6da1865a6084e\n\"d3d11\"=\"builtin\"\n\"winhttp\"=\"native,builtin\"\n",
        ));
        assert!(!merged.contains("\"d3d11\"=\"builtin\"\n\n\"winhttp\""));
    }

    #[test]
    fn merges_into_existing_dll_overrides_section_with_timestamp() {
        let content = r#"REGEDIT4

[Software\\Wine\\DllOverrides] 1608830137
#time=1d6da1865a6084e
"d3d11"="builtin"
"#;
        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains("\"d3d11\"=\"builtin\""));
        assert!(merged.contains("\"winhttp\"=\"native,builtin\""));
        assert!(is_winhttp_configured(&merged));
        assert_eq!(merged.matches("[Software\\\\Wine\\\\DllOverrides]").count(), 1);
    }

    #[test]
    fn merges_into_existing_dll_overrides_section() {
        let content = r#"REGEDIT4

[Software\\Wine\\DllOverrides]
"d3d11"="builtin"
"#;
        let merged = merge_winhttp_overrides(content);
        assert!(merged.contains("\"d3d11\"=\"builtin\""));
        assert!(is_winhttp_configured(&merged));
        assert!(!merged.contains("[Software\\Wine\\DllOverrides]\n[Software"));
    }

    #[test]
    fn manifest_matches_installdir_case_insensitive() {
        let content = "\"installdir\"\t\t\"zeepkist\"\n";
        assert!(manifest_matches_installdir(content, "Zeepkist"));
    }
}
