use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::bepinex::has_bepinex_structure;
use crate::fs_move::move_dir;
use crate::game_path::game_directory;
use crate::game_process::ensure_game_not_running;
use crate::mod_folder::sanitize_mod_name;
use crate::zip_extract::{extract_zip, sanitize_filename};

const SIDELOAD_ROOT: &str = "BepInEx/plugins/Sideloaded";
const PLUGINS_DIR: &str = "Plugins";
const BLUEPRINTS_DIR: &str = "Blueprints";
const INSPECT_TEMP_DIR: &str = "modkist-sideload-inspect";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SideloadTargetKind {
    Plugins,
    Blueprints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SideloadSourceType {
    Dll,
    Zeeplevel,
    Archive,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SideloadedEntry {
    pub id: String,
    pub name: String,
    pub target_kind: SideloadTargetKind,
    pub source_type: SideloadSourceType,
    pub added_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AddSideloadedModResult {
    Added { entry: SideloadedEntry },
    NeedsTargetChoice {
        folder_name: String,
        source_path: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveContentKind {
    PluginsOnly,
    BlueprintsOnly,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideloadFileKind {
    Dll,
    Zeeplevel,
    Archive,
}

fn sideload_root(game_dir: &Path) -> PathBuf {
    game_dir.join(SIDELOAD_ROOT)
}

fn sideload_kind_root(root: &Path, kind: SideloadTargetKind) -> PathBuf {
    let dir_name = match kind {
        SideloadTargetKind::Plugins => PLUGINS_DIR,
        SideloadTargetKind::Blueprints => BLUEPRINTS_DIR,
    };
    root.join(dir_name)
}

fn entry_id(kind: SideloadTargetKind, folder_name: &str) -> String {
    let kind_dir = match kind {
        SideloadTargetKind::Plugins => PLUGINS_DIR,
        SideloadTargetKind::Blueprints => BLUEPRINTS_DIR,
    };
    format!("{kind_dir}/{folder_name}")
}

fn format_mtime(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let datetime = OffsetDateTime::from(modified);
    datetime.format(&Rfc3339).ok()
}

fn is_safe_path_segment(segment: &str) -> bool {
    !segment.is_empty() && segment != "." && segment != ".."
}

fn is_safe_entry_id(entry_id: &str) -> bool {
    if entry_id.is_empty() || entry_id.contains('\\') {
        return false;
    }

    let parts: Vec<&str> = entry_id.split('/').collect();
    match parts.as_slice() {
        [name] => {
            is_safe_path_segment(name) && *name != PLUGINS_DIR && *name != BLUEPRINTS_DIR
        }
        [kind, name]
            if (*kind == PLUGINS_DIR || *kind == BLUEPRINTS_DIR) && is_safe_path_segment(name) =>
        {
            true
        }
        _ => false,
    }
}

fn resolve_entry_dir(root: &Path, entry_id: &str) -> PathBuf {
    root.join(entry_id)
}

fn file_kind_for_extension(extension: &str) -> Option<SideloadFileKind> {
    match extension.to_ascii_lowercase().as_str() {
        "dll" => Some(SideloadFileKind::Dll),
        "zeeplevel" => Some(SideloadFileKind::Zeeplevel),
        "zip" => Some(SideloadFileKind::Archive),
        _ => None,
    }
}

fn is_zeeplevel_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zeeplevel"))
}

fn folder_name_from_source(source_path: &Path) -> Result<String, String> {
    let stem = source_path
        .file_stem()
        .and_then(|name| name.to_str())
        .map(sanitize_mod_name)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "Could not derive a folder name from the selected file.".to_string())?;

    Ok(stem)
}

fn unique_folder_name(root: &Path, base_name: &str) -> String {
    let candidate = root.join(base_name);
    if !candidate.exists() {
        return base_name.to_string();
    }

    let mut suffix = 2;
    loop {
        let name = format!("{base_name}_{suffix}");
        if !root.join(&name).exists() {
            return name;
        }
        suffix += 1;
    }
}

fn temp_extract_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir()
        .join(INSPECT_TEMP_DIR)
        .join(format!("extract-{nanos}"))
}

fn scan_archive_contents(dir: &Path) -> Result<ArchiveContentKind, String> {
    let mut has_zeeplevel = false;
    let mut has_non_zeeplevel = false;
    let mut file_count = 0;

    fn walk(
        dir: &Path,
        has_zeeplevel: &mut bool,
        has_non_zeeplevel: &mut bool,
        file_count: &mut usize,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Could not read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Could not read entry type: {e}"))?;

            if file_type.is_dir() {
                walk(&path, has_zeeplevel, has_non_zeeplevel, file_count)?;
                continue;
            }

            *file_count += 1;
            if is_zeeplevel_path(&path) {
                *has_zeeplevel = true;
            } else {
                *has_non_zeeplevel = true;
            }
        }
        Ok(())
    }

    walk(dir, &mut has_zeeplevel, &mut has_non_zeeplevel, &mut file_count)?;

    if file_count == 0 {
        return Err("Archive contains no files.".into());
    }

    Ok(if has_zeeplevel && has_non_zeeplevel {
        ArchiveContentKind::Mixed
    } else if has_zeeplevel {
        ArchiveContentKind::BlueprintsOnly
    } else {
        ArchiveContentKind::PluginsOnly
    })
}

fn resolve_archive_target(
    content_kind: ArchiveContentKind,
    target_kind: Option<SideloadTargetKind>,
) -> Option<SideloadTargetKind> {
    match content_kind {
        ArchiveContentKind::PluginsOnly => Some(SideloadTargetKind::Plugins),
        ArchiveContentKind::BlueprintsOnly => Some(SideloadTargetKind::Blueprints),
        ArchiveContentKind::Mixed => target_kind,
    }
}

fn detect_source_type(entry_dir: &Path) -> Result<SideloadSourceType, String> {
    let mut dll_count = 0;
    let mut zeeplevel_count = 0;
    let mut other_count = 0;

    fn walk(
        dir: &Path,
        dll_count: &mut usize,
        zeeplevel_count: &mut usize,
        other_count: &mut usize,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Could not read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Could not read entry type: {e}"))?;

            if file_type.is_dir() {
                walk(&path, dll_count, zeeplevel_count, other_count)?;
                continue;
            }

            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            {
                *dll_count += 1;
            } else if is_zeeplevel_path(&path) {
                *zeeplevel_count += 1;
            } else {
                *other_count += 1;
            }
        }
        Ok(())
    }

    walk(entry_dir, &mut dll_count, &mut zeeplevel_count, &mut other_count)?;

    if dll_count == 1 && zeeplevel_count == 0 && other_count == 0 {
        Ok(SideloadSourceType::Dll)
    } else if zeeplevel_count == 1 && dll_count == 0 && other_count == 0 {
        Ok(SideloadSourceType::Zeeplevel)
    } else {
        Ok(SideloadSourceType::Archive)
    }
}

fn make_entry(
    target_kind: SideloadTargetKind,
    folder_name: &str,
    source_type: SideloadSourceType,
    entry_dir: &Path,
) -> SideloadedEntry {
    SideloadedEntry {
        id: entry_id(target_kind, folder_name),
        name: folder_name.to_string(),
        target_kind,
        source_type,
        added_at: format_mtime(entry_dir),
    }
}

fn scan_kind_entries(
    root: &Path,
    kind: SideloadTargetKind,
) -> Result<Vec<SideloadedEntry>, String> {
    let kind_root = sideload_kind_root(root, kind);
    if !kind_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&kind_root)
        .map_err(|e| format!("Could not read {}: {e}", kind_root.display()))?
    {
        let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
        if !entry
            .file_type()
            .map_err(|e| format!("Could not read entry type: {e}"))?
            .is_dir()
        {
            continue;
        }

        let folder_name = entry.file_name();
        let folder_name = folder_name.to_string_lossy();
        let entry_dir = kind_root.join(entry.file_name());
        entries.push(make_entry(
            kind,
            &folder_name,
            detect_source_type(&entry_dir)?,
            &entry_dir,
        ));
    }

    Ok(entries)
}

fn infer_legacy_target_kind(entry_dir: &Path) -> Result<SideloadTargetKind, String> {
    let mut has_zeeplevel = false;
    let mut has_dll = false;

    fn walk(dir: &Path, has_zeeplevel: &mut bool, has_dll: &mut bool) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Could not read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Could not read entry type: {e}"))?;

            if file_type.is_dir() {
                walk(&path, has_zeeplevel, has_dll)?;
                continue;
            }

            if is_zeeplevel_path(&path) {
                *has_zeeplevel = true;
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            {
                *has_dll = true;
            }
        }
        Ok(())
    }

    walk(entry_dir, &mut has_zeeplevel, &mut has_dll)?;

    if has_zeeplevel && !has_dll {
        Ok(SideloadTargetKind::Blueprints)
    } else {
        Ok(SideloadTargetKind::Plugins)
    }
}

fn scan_legacy_entries(root: &Path) -> Result<Vec<SideloadedEntry>, String> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| format!("Could not read {}: {e}", root.display()))? {
        let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
        if !entry
            .file_type()
            .map_err(|e| format!("Could not read entry type: {e}"))?
            .is_dir()
        {
            continue;
        }

        let folder_name = entry.file_name();
        let folder_name = folder_name.to_string_lossy();
        if folder_name == PLUGINS_DIR || folder_name == BLUEPRINTS_DIR {
            continue;
        }

        let entry_dir = root.join(entry.file_name());
        let target_kind = infer_legacy_target_kind(&entry_dir)?;
        entries.push(SideloadedEntry {
            id: folder_name.to_string(),
            name: folder_name.to_string(),
            target_kind,
            source_type: detect_source_type(&entry_dir)?,
            added_at: format_mtime(&entry_dir),
        });
    }

    Ok(entries)
}

fn list_all_entries(root: &Path) -> Result<Vec<SideloadedEntry>, String> {
    let mut entries = scan_kind_entries(root, SideloadTargetKind::Plugins)?;
    entries.extend(scan_kind_entries(root, SideloadTargetKind::Blueprints)?);
    entries.extend(scan_legacy_entries(root)?);
    entries.sort_by(|left, right| left.name.to_ascii_lowercase().cmp(&right.name.to_ascii_lowercase()));
    Ok(entries)
}

fn ensure_sideload_ready(app: &AppHandle) -> Result<PathBuf, String> {
    let game_dir = game_directory(app)?;
    if !has_bepinex_structure(&game_dir) {
        return Err("BepInEx is not installed in your game directory.".into());
    }
    Ok(sideload_root(&game_dir))
}

fn install_single_file(
    source_path: &Path,
    kind_root: &Path,
    base_name: &str,
    target_kind: SideloadTargetKind,
    source_type: SideloadSourceType,
) -> Result<SideloadedEntry, String> {
    fs::create_dir_all(kind_root).map_err(|e| {
        format!(
            "Could not create sideload directory {}: {e}",
            kind_root.display()
        )
    })?;

    let folder_name = unique_folder_name(kind_root, base_name);
    let destination = kind_root.join(&folder_name);
    fs::create_dir_all(&destination).map_err(|e| {
        format!(
            "Could not create sideload entry directory {}: {e}",
            destination.display()
        )
    })?;

    let file_name = sanitize_filename(
        source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("mod"),
    );
    let dest_path = destination.join(&file_name);
    fs::copy(source_path, &dest_path).map_err(|e| {
        format!(
            "Could not copy file to {}: {e}",
            dest_path.display()
        )
    })?;

    Ok(make_entry(target_kind, &folder_name, source_type, &destination))
}

fn install_extracted_archive(
    temp_dir: &Path,
    kind_root: &Path,
    base_name: &str,
    target_kind: SideloadTargetKind,
) -> Result<SideloadedEntry, String> {
    fs::create_dir_all(kind_root).map_err(|e| {
        format!(
            "Could not create sideload directory {}: {e}",
            kind_root.display()
        )
    })?;

    let folder_name = unique_folder_name(kind_root, base_name);
    let destination = kind_root.join(&folder_name);
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|e| {
            format!(
                "Could not replace existing sideload entry {}: {e}",
                destination.display()
            )
        })?;
    }

    move_dir(temp_dir, &destination)?;
    Ok(make_entry(
        target_kind,
        &folder_name,
        SideloadSourceType::Archive,
        &destination,
    ))
}

fn with_temp_archive_extract<T, F>(source_path: &Path, operation: F) -> Result<T, String>
where
    F: FnOnce(&Path) -> Result<T, String>,
{
    let temp_dir = temp_extract_dir();
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|e| {
            format!(
                "Could not clear temp directory {}: {e}",
                temp_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&temp_dir).map_err(|e| {
        format!(
            "Could not create temp directory {}: {e}",
            temp_dir.display()
        )
    })?;

    let result = extract_zip(source_path, &temp_dir).and_then(|_| operation(&temp_dir));
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    result
}

#[tauri::command]
pub fn list_sideloaded_mods(app: AppHandle) -> Result<Vec<SideloadedEntry>, String> {
    let root = ensure_sideload_ready(&app)?;
    list_all_entries(&root)
}

#[tauri::command]
pub fn add_sideloaded_mod(
    app: AppHandle,
    source_path: String,
    target_kind: Option<SideloadTargetKind>,
) -> Result<AddSideloadedModResult, String> {
    ensure_game_not_running()?;

    let source_path = PathBuf::from(&source_path);
    if !source_path.is_file() {
        return Err("Selected file does not exist.".into());
    }

    let extension = source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| {
            "Selected file must be a .dll, .zeeplevel, or .zip file.".to_string()
        })?;
    let file_kind = file_kind_for_extension(extension).ok_or_else(|| {
        "Only .dll, .zeeplevel, and .zip files can be sideloaded.".to_string()
    })?;

    let root = ensure_sideload_ready(&app)?;
    fs::create_dir_all(&root).map_err(|e| {
        format!(
            "Could not create sideload directory {}: {e}",
            root.display()
        )
    })?;

    let base_name = folder_name_from_source(&source_path)?;

    match file_kind {
        SideloadFileKind::Dll => {
            let kind_root = sideload_kind_root(&root, SideloadTargetKind::Plugins);
            let entry = install_single_file(
                &source_path,
                &kind_root,
                &base_name,
                SideloadTargetKind::Plugins,
                SideloadSourceType::Dll,
            )?;
            Ok(AddSideloadedModResult::Added { entry })
        }
        SideloadFileKind::Zeeplevel => {
            let kind_root = sideload_kind_root(&root, SideloadTargetKind::Blueprints);
            let entry = install_single_file(
                &source_path,
                &kind_root,
                &base_name,
                SideloadTargetKind::Blueprints,
                SideloadSourceType::Zeeplevel,
            )?;
            Ok(AddSideloadedModResult::Added { entry })
        }
        SideloadFileKind::Archive => with_temp_archive_extract(&source_path, |temp_dir| {
            let content_kind = scan_archive_contents(temp_dir)?;
            let Some(resolved_target) = resolve_archive_target(content_kind, target_kind) else {
                return Ok(AddSideloadedModResult::NeedsTargetChoice {
                    folder_name: base_name.clone(),
                    source_path: source_path.to_string_lossy().into_owned(),
                });
            };

            let kind_root = sideload_kind_root(&root, resolved_target);
            let entry =
                install_extracted_archive(temp_dir, &kind_root, &base_name, resolved_target)?;
            Ok(AddSideloadedModResult::Added { entry })
        }),
    }
}

#[tauri::command]
pub fn remove_sideloaded_mod(
    app: AppHandle,
    entry_id: String,
) -> Result<Vec<SideloadedEntry>, String> {
    ensure_game_not_running()?;

    if !is_safe_entry_id(&entry_id) {
        return Err("Invalid sideload entry id.".into());
    }

    let root = ensure_sideload_ready(&app)?;
    let entry_dir = resolve_entry_dir(&root, &entry_id);
    if !entry_dir.is_dir() {
        return Err("Sideloaded mod was not found.".into());
    }

    fs::remove_dir_all(&entry_dir).map_err(|e| {
        format!(
            "Could not remove sideload entry {}: {e}",
            entry_dir.display()
        )
    })?;

    list_all_entries(&root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn rejects_unsafe_entry_ids() {
        assert!(!is_safe_entry_id(""));
        assert!(!is_safe_entry_id(".."));
        assert!(!is_safe_entry_id("../escape"));
        assert!(!is_safe_entry_id(r"foo\bar"));
        assert!(!is_safe_entry_id("Plugins"));
        assert!(!is_safe_entry_id("Plugins/foo/extra"));
        assert!(is_safe_entry_id("MyMod"));
        assert!(is_safe_entry_id("Plugins/MyMod"));
        assert!(is_safe_entry_id("Blueprints/MyLevel"));
    }

    #[test]
    fn assigns_unique_folder_names() {
        let root = std::env::temp_dir().join("modkist-sideload-unique");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("CoolMod")).unwrap();
        fs::create_dir_all(root.join("CoolMod_2")).unwrap();

        assert_eq!(unique_folder_name(&root, "CoolMod"), "CoolMod_3");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn classifies_archive_contents() {
        let root = std::env::temp_dir().join("modkist-sideload-classify");
        let _ = fs::remove_dir_all(&root);

        let plugins_only = root.join("plugins-only");
        fs::create_dir_all(&plugins_only).unwrap();
        fs::write(plugins_only.join("mod.dll"), b"dll").unwrap();
        assert_eq!(
            scan_archive_contents(&plugins_only).unwrap(),
            ArchiveContentKind::PluginsOnly
        );

        let blueprints_only = root.join("blueprints-only");
        fs::create_dir_all(&blueprints_only).unwrap();
        fs::write(blueprints_only.join("level.zeeplevel"), b"level").unwrap();
        assert_eq!(
            scan_archive_contents(&blueprints_only).unwrap(),
            ArchiveContentKind::BlueprintsOnly
        );

        let mixed = root.join("mixed");
        fs::create_dir_all(&mixed).unwrap();
        fs::write(mixed.join("mod.dll"), b"dll").unwrap();
        fs::write(mixed.join("level.zeeplevel"), b"level").unwrap();
        assert_eq!(
            scan_archive_contents(&mixed).unwrap(),
            ArchiveContentKind::Mixed
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_kind_and_legacy_entries() {
        let root = std::env::temp_dir().join("modkist-sideload-list");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Plugins/TestPlugin")).unwrap();
        fs::write(root.join("Plugins/TestPlugin/mod.dll"), b"dll").unwrap();
        fs::create_dir_all(root.join("Blueprints/TestBlueprint")).unwrap();
        fs::write(
            root.join("Blueprints/TestBlueprint/level.zeeplevel"),
            b"level",
        )
        .unwrap();
        fs::create_dir_all(root.join("LegacyMod")).unwrap();
        fs::write(root.join("LegacyMod/mod.dll"), b"dll").unwrap();

        let entries = list_all_entries(&root).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|entry| entry.id == "Plugins/TestPlugin"));
        assert!(entries
            .iter()
            .any(|entry| entry.id == "Blueprints/TestBlueprint"));
        assert!(entries.iter().any(|entry| entry.id == "LegacyMod"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sideload_round_trip_for_dll_and_zip() {
        let root = std::env::temp_dir().join("modkist-sideload-roundtrip");
        let _ = fs::remove_dir_all(&root);
        let sideload_dir = root.join("Sideloaded");
        let plugins_dir = sideload_dir.join("Plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        let dll_source = root.join("TestMod.dll");
        fs::write(&dll_source, b"fake dll").unwrap();
        let entry = install_single_file(
            &dll_source,
            &plugins_dir,
            "TestMod",
            SideloadTargetKind::Plugins,
            SideloadSourceType::Dll,
        )
        .unwrap();
        assert_eq!(entry.id, "Plugins/TestMod");

        let zip_source = root.join("ArchiveMod.zip");
        let zip_file = fs::File::create(&zip_source).unwrap();
        let mut zip = zip::ZipWriter::new(zip_file);
        zip.start_file("plugin.dll", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"zip dll").unwrap();
        zip.finish().unwrap();

        let temp_dir = root.join("temp-archive");
        fs::create_dir_all(&temp_dir).unwrap();
        extract_zip(&zip_source, &temp_dir).unwrap();
        let entry = install_extracted_archive(
            &temp_dir,
            &plugins_dir,
            "ArchiveMod",
            SideloadTargetKind::Plugins,
        )
        .unwrap();
        assert_eq!(entry.id, "Plugins/ArchiveMod");

        let entries = list_all_entries(&sideload_dir).unwrap();
        assert_eq!(entries.len(), 2);

        fs::remove_dir_all(plugins_dir.join("TestMod")).unwrap();
        let entries = list_all_entries(&sideload_dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "Plugins/ArchiveMod");

        let _ = fs::remove_dir_all(&root);
    }
}
