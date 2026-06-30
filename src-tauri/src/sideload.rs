use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::bepinex::has_bepinex_structure;
use crate::game_path::game_directory;
use crate::game_process::ensure_game_not_running;
use crate::mod_folder::sanitize_mod_name;
use crate::zip_extract::{extract_zip, sanitize_filename};

const SIDELOAD_ROOT: &str = "BepInEx/plugins/Sideloaded";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SideloadSourceType {
    Dll,
    Archive,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SideloadedEntry {
    pub id: String,
    pub name: String,
    pub source_type: SideloadSourceType,
    pub added_at: Option<String>,
}

fn sideload_root(game_dir: &Path) -> PathBuf {
    game_dir.join(SIDELOAD_ROOT)
}

fn format_mtime(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let datetime = OffsetDateTime::from(modified);
    datetime.format(&Rfc3339).ok()
}

fn is_safe_entry_id(entry_id: &str) -> bool {
    if entry_id.is_empty() || entry_id == "." || entry_id == ".." {
        return false;
    }

    if entry_id.contains(['/', '\\']) {
        return false;
    }

    Path::new(entry_id)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn source_type_for_extension(extension: &str) -> Option<SideloadSourceType> {
    match extension.to_ascii_lowercase().as_str() {
        "dll" => Some(SideloadSourceType::Dll),
        "zip" => Some(SideloadSourceType::Archive),
        _ => None,
    }
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

fn scan_entry(root: &Path, folder_name: &str) -> Result<SideloadedEntry, String> {
    let entry_dir = root.join(folder_name);
    let source_type = detect_source_type(&entry_dir)?;

    Ok(SideloadedEntry {
        id: folder_name.to_string(),
        name: folder_name.to_string(),
        source_type,
        added_at: format_mtime(&entry_dir),
    })
}

fn detect_source_type(entry_dir: &Path) -> Result<SideloadSourceType, String> {
    let mut has_dll = false;
    let mut has_zip_marker = false;

    fn walk(dir: &Path, has_dll: &mut bool, has_zip_marker: &mut bool) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Could not read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Could not read entry type: {e}"))?;

            if file_type.is_dir() {
                walk(&path, has_dll, has_zip_marker)?;
                continue;
            }

            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            {
                *has_dll = true;
            } else {
                *has_zip_marker = true;
            }
        }
        Ok(())
    }

    walk(entry_dir, &mut has_dll, &mut has_zip_marker)?;

    if has_dll && !has_zip_marker {
        Ok(SideloadSourceType::Dll)
    } else {
        Ok(SideloadSourceType::Archive)
    }
}

fn list_entries_in_root(root: &Path) -> Result<Vec<SideloadedEntry>, String> {
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
        entries.push(scan_entry(root, &folder_name)?);
    }

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

#[tauri::command]
pub fn list_sideloaded_mods(app: AppHandle) -> Result<Vec<SideloadedEntry>, String> {
    let root = ensure_sideload_ready(&app)?;
    list_entries_in_root(&root)
}

#[tauri::command]
pub fn add_sideloaded_mod(app: AppHandle, source_path: String) -> Result<SideloadedEntry, String> {
    ensure_game_not_running()?;

    let source_path = PathBuf::from(&source_path);
    if !source_path.is_file() {
        return Err("Selected file does not exist.".into());
    }

    let extension = source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| "Selected file must be a .dll or .zip file.".to_string())?;
    let source_type = source_type_for_extension(extension)
        .ok_or_else(|| "Only .dll and .zip files can be sideloaded.".to_string())?;

    let root = ensure_sideload_ready(&app)?;
    fs::create_dir_all(&root).map_err(|e| {
        format!(
            "Could not create sideload directory {}: {e}",
            root.display()
        )
    })?;

    let base_name = folder_name_from_source(&source_path)?;
    let folder_name = unique_folder_name(&root, &base_name);
    let destination = root.join(&folder_name);
    fs::create_dir_all(&destination).map_err(|e| {
        format!(
            "Could not create sideload entry directory {}: {e}",
            destination.display()
        )
    })?;

    match source_type {
        SideloadSourceType::Dll => {
            let file_name = sanitize_filename(
                source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("mod.dll"),
            );
            let dest_path = destination.join(&file_name);
            fs::copy(&source_path, &dest_path).map_err(|e| {
                format!(
                    "Could not copy DLL to {}: {e}",
                    dest_path.display()
                )
            })?;
        }
        SideloadSourceType::Archive => {
            extract_zip(&source_path, &destination)?;
        }
    }

    scan_entry(&root, &folder_name)
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
    let entry_dir = root.join(&entry_id);
    if !entry_dir.is_dir() {
        return Err("Sideloaded mod was not found.".into());
    }

    fs::remove_dir_all(&entry_dir).map_err(|e| {
        format!(
            "Could not remove sideload entry {}: {e}",
            entry_dir.display()
        )
    })?;

    list_entries_in_root(&root)
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
        assert!(is_safe_entry_id("MyMod"));
        assert!(is_safe_entry_id("MyMod_2"));
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
    fn sideload_round_trip_for_dll_and_zip() {
        let root = std::env::temp_dir().join("modkist-sideload-roundtrip");
        let _ = fs::remove_dir_all(&root);
        let sideload_dir = root.join("Sideloaded");
        fs::create_dir_all(&sideload_dir).unwrap();

        let dll_source = root.join("TestMod.dll");
        fs::write(&dll_source, b"fake dll").unwrap();
        let base_name = folder_name_from_source(&dll_source).unwrap();
        let folder_name = unique_folder_name(&sideload_dir, &base_name);
        let destination = sideload_dir.join(&folder_name);
        fs::create_dir_all(&destination).unwrap();
        fs::copy(&dll_source, destination.join("TestMod.dll")).unwrap();

        let entries = list_entries_in_root(&sideload_dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "TestMod");
        assert_eq!(entries[0].source_type, SideloadSourceType::Dll);

        let zip_source = root.join("ArchiveMod.zip");
        let zip_file = fs::File::create(&zip_source).unwrap();
        let mut zip = zip::ZipWriter::new(zip_file);
        zip.start_file("plugin.dll", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"zip dll").unwrap();
        zip.finish().unwrap();

        let archive_folder = unique_folder_name(&sideload_dir, "ArchiveMod");
        let archive_destination = sideload_dir.join(&archive_folder);
        fs::create_dir_all(&archive_destination).unwrap();
        extract_zip(&zip_source, &archive_destination).unwrap();

        let entries = list_entries_in_root(&sideload_dir).unwrap();
        assert_eq!(entries.len(), 2);

        fs::remove_dir_all(sideload_dir.join("TestMod")).unwrap();
        let entries = list_entries_in_root(&sideload_dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "ArchiveMod");

        let _ = fs::remove_dir_all(&root);
    }
}
