use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::bepinex::has_bepinex_structure;
use crate::fs_link::symlink_file;
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
    pub linked: bool,
    pub added_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AddSideloadedModResult {
    Added { entry: SideloadedEntry },
    #[serde(rename_all = "camelCase")]
    NeedsTargetChoice {
        folder_name: String,
        source_paths: Vec<String>,
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

fn path_is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

fn directory_contains_symlink(dir: &Path) -> bool {
    fn walk(dir: &Path) -> bool {
        let Ok(entries) = fs::read_dir(dir) else {
            return false;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path_is_symlink(&path) {
                return true;
            }

            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() && walk(&path) {
                return true;
            }
        }

        false
    }

    walk(dir)
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

fn is_dll_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

fn loose_file_source_type(path: &Path, kind: SideloadTargetKind) -> Option<SideloadSourceType> {
    root_loose_file_kind(path).and_then(|(file_kind, source_type)| {
        if file_kind == kind {
            Some(source_type)
        } else {
            None
        }
    })
}

fn root_loose_file_kind(path: &Path) -> Option<(SideloadTargetKind, SideloadSourceType)> {
    if is_dll_path(path) {
        Some((SideloadTargetKind::Plugins, SideloadSourceType::Dll))
    } else if is_zeeplevel_path(path) {
        Some((SideloadTargetKind::Blueprints, SideloadSourceType::Zeeplevel))
    } else {
        None
    }
}

fn folder_name_from_source(source_path: &Path) -> Result<String, String> {
    let stem = source_path
        .file_stem()
        .and_then(|name| name.to_str())
        .map(sanitize_mod_name)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "Did not derive a folder name from the selected file.".to_string())?;

    Ok(stem)
}

fn folder_name_from_sources(source_paths: &[PathBuf]) -> Result<String, String> {
    if source_paths.is_empty() {
        return Err("No files were selected.".into());
    }

    if source_paths.len() == 1 {
        return folder_name_from_source(&source_paths[0]);
    }

    let parents: Vec<_> = source_paths
        .iter()
        .filter_map(|path| path.parent())
        .collect();

    if parents.len() == source_paths.len() {
        let first_parent = parents[0];
        if parents.iter().all(|parent| *parent == first_parent) {
            if let Some(name) = first_parent
                .file_name()
                .and_then(|name| name.to_str())
                .map(sanitize_mod_name)
                .filter(|name| !name.is_empty())
            {
                return Ok(name);
            }
        }
    }

    folder_name_from_source(&source_paths[0])
}

fn classify_loose_files(source_paths: &[PathBuf]) -> Result<ArchiveContentKind, String> {
    let mut has_dll = false;
    let mut has_zeeplevel = false;

    for path in source_paths {
        if is_dll_path(path) {
            has_dll = true;
        } else if is_zeeplevel_path(path) {
            has_zeeplevel = true;
        } else {
            return Err(
                "Only .dll and .zeeplevel files can be sideloaded together.".into(),
            );
        }
    }

    Ok(if has_dll && has_zeeplevel {
        ArchiveContentKind::Mixed
    } else if has_zeeplevel {
        ArchiveContentKind::BlueprintsOnly
    } else {
        ArchiveContentKind::PluginsOnly
    })
}

/// Classify files for symlink installs. Any non-dll/non-zeeplevel file (or a
/// mix of dll + zeeplevel) requires an explicit target choice.
fn classify_link_files(source_paths: &[PathBuf]) -> ArchiveContentKind {
    let mut has_dll = false;
    let mut has_zeeplevel = false;
    let mut has_other = false;

    for path in source_paths {
        if is_dll_path(path) {
            has_dll = true;
        } else if is_zeeplevel_path(path) {
            has_zeeplevel = true;
        } else {
            has_other = true;
        }
    }

    if has_other || (has_dll && has_zeeplevel) {
        ArchiveContentKind::Mixed
    } else if has_zeeplevel {
        ArchiveContentKind::BlueprintsOnly
    } else {
        ArchiveContentKind::PluginsOnly
    }
}

fn unique_dest_file_name(destination: &Path, file_name: &str) -> String {
    let candidate = destination.join(file_name);
    if !candidate.exists() {
        return file_name.to_string();
    }

    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let extension = path.extension().and_then(|ext| ext.to_str());

    let mut suffix = 2;
    loop {
        let name = match extension {
            Some(ext) => format!("{stem}_{suffix}.{ext}"),
            None => format!("{stem}_{suffix}"),
        };
        if !destination.join(&name).exists() {
            return name;
        }
        suffix += 1;
    }
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
        for entry in fs::read_dir(dir).map_err(|e| format!("Did not read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("Did not read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Did not read entry type: {e}"))?;

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
        for entry in fs::read_dir(dir).map_err(|e| format!("Did not read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("Did not read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Did not read entry type: {e}"))?;

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
    linked: bool,
) -> SideloadedEntry {
    SideloadedEntry {
        id: entry_id(target_kind, folder_name),
        name: folder_name.to_string(),
        target_kind,
        source_type,
        linked,
        added_at: format_mtime(entry_dir),
    }
}

fn make_loose_file_entry(
    target_kind: SideloadTargetKind,
    file_name: &str,
    source_type: SideloadSourceType,
    file_path: &Path,
) -> SideloadedEntry {
    let name = file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(file_name)
        .to_string();

    SideloadedEntry {
        id: entry_id(target_kind, file_name),
        name,
        target_kind,
        source_type,
        linked: path_is_symlink(file_path),
        added_at: format_mtime(file_path),
    }
}

fn make_root_loose_file_entry(
    file_name: &str,
    target_kind: SideloadTargetKind,
    source_type: SideloadSourceType,
    file_path: &Path,
) -> SideloadedEntry {
    let name = file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(file_name)
        .to_string();

    SideloadedEntry {
        id: file_name.to_string(),
        name,
        target_kind,
        source_type,
        linked: path_is_symlink(file_path),
        added_at: format_mtime(file_path),
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
        .map_err(|e| format!("Did not read {}: {e}", kind_root.display()))?
    {
        let entry = entry.map_err(|e| format!("Did not read directory entry: {e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Did not read entry type: {e}"))?;
        let path = entry.path();

        if file_type.is_dir() {
            let folder_name = entry.file_name();
            let folder_name = folder_name.to_string_lossy();
            entries.push(make_entry(
                kind,
                &folder_name,
                detect_source_type(&path)?,
                &path,
                directory_contains_symlink(&path),
            ));
            continue;
        }

        if file_type.is_file() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if let Some(source_type) = loose_file_source_type(&path, kind) {
                entries.push(make_loose_file_entry(
                    kind,
                    &file_name,
                    source_type,
                    &path,
                ));
            }
        }
    }

    Ok(entries)
}

fn infer_legacy_target_kind(entry_dir: &Path) -> Result<SideloadTargetKind, String> {
    let mut has_zeeplevel = false;
    let mut has_dll = false;

    fn walk(dir: &Path, has_zeeplevel: &mut bool, has_dll: &mut bool) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Did not read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("Did not read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Did not read entry type: {e}"))?;

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
    for entry in fs::read_dir(root).map_err(|e| format!("Did not read {}: {e}", root.display()))? {
        let entry = entry.map_err(|e| format!("Did not read directory entry: {e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Did not read entry type: {e}"))?;
        let path = entry.path();

        if file_type.is_dir() {
            let folder_name = entry.file_name();
            let folder_name = folder_name.to_string_lossy();
            if folder_name == PLUGINS_DIR || folder_name == BLUEPRINTS_DIR {
                continue;
            }

            entries.push(SideloadedEntry {
                id: folder_name.to_string(),
                name: folder_name.to_string(),
                target_kind: infer_legacy_target_kind(&path)?,
                source_type: detect_source_type(&path)?,
                linked: directory_contains_symlink(&path),
                added_at: format_mtime(&path),
            });
            continue;
        }

        if file_type.is_file() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if let Some((target_kind, source_type)) = root_loose_file_kind(&path) {
                entries.push(make_root_loose_file_entry(
                    &file_name,
                    target_kind,
                    source_type,
                    &path,
                ));
            }
        }
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

fn place_file(source_path: &Path, dest_path: &Path, use_symlinks: bool) -> Result<(), String> {
    if use_symlinks {
        symlink_file(source_path, dest_path)
    } else {
        fs::copy(source_path, dest_path).map_err(|e| {
            format!(
                "Did not copy file to {}: {e}",
                dest_path.display()
            )
        })?;
        Ok(())
    }
}

fn install_single_file(
    source_path: &Path,
    kind_root: &Path,
    base_name: &str,
    target_kind: SideloadTargetKind,
    source_type: SideloadSourceType,
    use_symlinks: bool,
) -> Result<SideloadedEntry, String> {
    fs::create_dir_all(kind_root).map_err(|e| {
        format!(
            "Did not create sideload directory {}: {e}",
            kind_root.display()
        )
    })?;

    let folder_name = unique_folder_name(kind_root, base_name);
    let destination = kind_root.join(&folder_name);
    fs::create_dir_all(&destination).map_err(|e| {
        format!(
            "Did not create sideload entry directory {}: {e}",
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
    place_file(source_path, &dest_path, use_symlinks)?;

    Ok(make_entry(
        target_kind,
        &folder_name,
        source_type,
        &destination,
        use_symlinks,
    ))
}

fn install_extracted_archive(
    temp_dir: &Path,
    kind_root: &Path,
    base_name: &str,
    target_kind: SideloadTargetKind,
) -> Result<SideloadedEntry, String> {
    fs::create_dir_all(kind_root).map_err(|e| {
        format!(
            "Did not create sideload directory {}: {e}",
            kind_root.display()
        )
    })?;

    let folder_name = unique_folder_name(kind_root, base_name);
    let destination = kind_root.join(&folder_name);
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|e| {
            format!(
                "Did not replace existing sideload entry {}: {e}",
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
        false,
    ))
}

fn install_loose_files(
    source_paths: &[PathBuf],
    kind_root: &Path,
    base_name: &str,
    target_kind: SideloadTargetKind,
    use_symlinks: bool,
) -> Result<SideloadedEntry, String> {
    fs::create_dir_all(kind_root).map_err(|e| {
        format!(
            "Did not create sideload directory {}: {e}",
            kind_root.display()
        )
    })?;

    let folder_name = unique_folder_name(kind_root, base_name);
    let destination = kind_root.join(&folder_name);
    fs::create_dir_all(&destination).map_err(|e| {
        format!(
            "Did not create sideload entry directory {}: {e}",
            destination.display()
        )
    })?;

    for source_path in source_paths {
        let file_name = sanitize_filename(
            source_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("mod"),
        );
        let dest_name = unique_dest_file_name(&destination, &file_name);
        let dest_path = destination.join(&dest_name);
        place_file(source_path, &dest_path, use_symlinks)?;
    }

    let source_type = if source_paths.len() == 1 {
        detect_source_type(&destination)?
    } else {
        SideloadSourceType::Archive
    };

    Ok(make_entry(
        target_kind,
        &folder_name,
        source_type,
        &destination,
        use_symlinks,
    ))
}

fn add_single_sideloaded_mod(
    root: &Path,
    source_path: &Path,
    target_kind: Option<SideloadTargetKind>,
    use_symlinks: bool,
) -> Result<AddSideloadedModResult, String> {
    if !source_path.is_file() {
        return Err("Selected file does not exist.".into());
    }

    if use_symlinks {
        return add_single_linked_mod(root, source_path, target_kind);
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

    let base_name = folder_name_from_source(source_path)?;
    let source_path_string = source_path.to_string_lossy().into_owned();

    match file_kind {
        SideloadFileKind::Dll => {
            let kind_root = sideload_kind_root(root, SideloadTargetKind::Plugins);
            let entry = install_single_file(
                source_path,
                &kind_root,
                &base_name,
                SideloadTargetKind::Plugins,
                SideloadSourceType::Dll,
                false,
            )?;
            Ok(AddSideloadedModResult::Added { entry })
        }
        SideloadFileKind::Zeeplevel => {
            let kind_root = sideload_kind_root(root, SideloadTargetKind::Blueprints);
            let entry = install_single_file(
                source_path,
                &kind_root,
                &base_name,
                SideloadTargetKind::Blueprints,
                SideloadSourceType::Zeeplevel,
                false,
            )?;
            Ok(AddSideloadedModResult::Added { entry })
        }
        SideloadFileKind::Archive => with_temp_archive_extract(source_path, |temp_dir| {
            let content_kind = scan_archive_contents(temp_dir)?;
            let Some(resolved_target) = resolve_archive_target(content_kind, target_kind) else {
                return Ok(AddSideloadedModResult::NeedsTargetChoice {
                    folder_name: base_name.clone(),
                    source_paths: vec![source_path_string.clone()],
                });
            };

            let kind_root = sideload_kind_root(root, resolved_target);
            let entry =
                install_extracted_archive(temp_dir, &kind_root, &base_name, resolved_target)?;
            Ok(AddSideloadedModResult::Added { entry })
        }),
    }
}

fn add_single_linked_mod(
    root: &Path,
    source_path: &Path,
    target_kind: Option<SideloadTargetKind>,
) -> Result<AddSideloadedModResult, String> {
    if is_zip_path(source_path) {
        return Err(
            "Zip archives must not be linked. Use Choose files to copy and extract archives.".into(),
        );
    }

    let base_name = folder_name_from_source(source_path)?;
    let source_path_string = source_path.to_string_lossy().into_owned();
    let extension = source_path.extension().and_then(|ext| ext.to_str());
    let file_kind = extension.and_then(file_kind_for_extension);

    match file_kind {
        Some(SideloadFileKind::Dll) => {
            let kind_root = sideload_kind_root(root, SideloadTargetKind::Plugins);
            let entry = install_single_file(
                source_path,
                &kind_root,
                &base_name,
                SideloadTargetKind::Plugins,
                SideloadSourceType::Dll,
                true,
            )?;
            Ok(AddSideloadedModResult::Added { entry })
        }
        Some(SideloadFileKind::Zeeplevel) => {
            let kind_root = sideload_kind_root(root, SideloadTargetKind::Blueprints);
            let entry = install_single_file(
                source_path,
                &kind_root,
                &base_name,
                SideloadTargetKind::Blueprints,
                SideloadSourceType::Zeeplevel,
                true,
            )?;
            Ok(AddSideloadedModResult::Added { entry })
        }
        _ => {
            let Some(resolved_target) = target_kind else {
                return Ok(AddSideloadedModResult::NeedsTargetChoice {
                    folder_name: base_name,
                    source_paths: vec![source_path_string],
                });
            };

            let kind_root = sideload_kind_root(root, resolved_target);
            let entry = install_single_file(
                source_path,
                &kind_root,
                &base_name,
                resolved_target,
                SideloadSourceType::Archive,
                true,
            )?;
            Ok(AddSideloadedModResult::Added { entry })
        }
    }
}

fn add_multi_sideloaded_mod(
    root: &Path,
    source_paths: &[PathBuf],
    target_kind: Option<SideloadTargetKind>,
    use_symlinks: bool,
) -> Result<AddSideloadedModResult, String> {
    for path in source_paths {
        if !path.is_file() {
            return Err(format!("Selected file does not exist: {}", path.display()));
        }

        if use_symlinks {
            if is_zip_path(path) {
                return Err(
                    "Zip archives must not be linked. Use Choose files to copy and extract archives.".into(),
                );
            }
            continue;
        }

        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| {
                "Selected files must be .dll or .zeeplevel files.".to_string()
            })?;
        let file_kind = file_kind_for_extension(extension).ok_or_else(|| {
            "Only .dll and .zeeplevel files can be sideloaded together.".to_string()
        })?;

        if file_kind == SideloadFileKind::Archive {
            return Err(
                "Zip archives must be sideloaded one at a time. Drop the zip alone, or select only .dll and .zeeplevel files.".into(),
            );
        }
    }

    let content_kind = if use_symlinks {
        classify_link_files(source_paths)
    } else {
        classify_loose_files(source_paths)?
    };
    let base_name = folder_name_from_sources(source_paths)?;
    let Some(resolved_target) = resolve_archive_target(content_kind, target_kind) else {
        return Ok(AddSideloadedModResult::NeedsTargetChoice {
            folder_name: base_name,
            source_paths: source_paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        });
    };

    let kind_root = sideload_kind_root(root, resolved_target);
    let entry = install_loose_files(
        source_paths,
        &kind_root,
        &base_name,
        resolved_target,
        use_symlinks,
    )?;
    Ok(AddSideloadedModResult::Added { entry })
}

fn with_temp_archive_extract<T, F>(source_path: &Path, operation: F) -> Result<T, String>
where
    F: FnOnce(&Path) -> Result<T, String>,
{
    let temp_dir = temp_extract_dir();
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|e| {
            format!(
                "Did not clear temp directory {}: {e}",
                temp_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&temp_dir).map_err(|e| {
        format!(
            "Did not create temp directory {}: {e}",
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
    source_paths: Vec<String>,
    target_kind: Option<SideloadTargetKind>,
    use_symlinks: Option<bool>,
) -> Result<AddSideloadedModResult, String> {
    ensure_game_not_running()?;

    if source_paths.is_empty() {
        return Err("No files were selected.".into());
    }

    let use_symlinks = use_symlinks.unwrap_or(false);
    let paths: Vec<PathBuf> = source_paths.iter().map(PathBuf::from).collect();

    let root = ensure_sideload_ready(&app)?;
    fs::create_dir_all(&root).map_err(|e| {
        format!(
            "Did not create sideload directory {}: {e}",
            root.display()
        )
    })?;

    if paths.len() == 1 {
        add_single_sideloaded_mod(&root, &paths[0], target_kind, use_symlinks)
    } else {
        add_multi_sideloaded_mod(&root, &paths, target_kind, use_symlinks)
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
    let entry_path = resolve_entry_dir(&root, &entry_id);
    if entry_path.is_file() {
        fs::remove_file(&entry_path).map_err(|e| {
            format!(
                "Did not remove sideload entry {}: {e}",
                entry_path.display()
            )
        })?;
    } else if entry_path.is_dir() {
        fs::remove_dir_all(&entry_path).map_err(|e| {
            format!(
                "Did not remove sideload entry {}: {e}",
                entry_path.display()
            )
        })?;
    } else {
        return Err("Sideloaded mod was not found.".into());
    }

    list_all_entries(&root)
}

#[tauri::command]
pub fn sideloaded_mod_path(app: AppHandle, entry_id: String) -> Result<String, String> {
    if !is_safe_entry_id(&entry_id) {
        return Err("Invalid sideload entry id.".into());
    }

    let root = ensure_sideload_ready(&app)?;
    let entry_path = resolve_entry_dir(&root, &entry_id);
    if !entry_path.exists() {
        return Err("Sideloaded mod was not found.".into());
    }

    Ok(entry_path.to_string_lossy().into_owned())
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
        assert!(is_safe_entry_id("MyMod.dll"));
        assert!(is_safe_entry_id("Plugins/MyMod"));
        assert!(is_safe_entry_id("Plugins/MyMod.dll"));
        assert!(is_safe_entry_id("Blueprints/MyLevel"));
        assert!(is_safe_entry_id("Blueprints/MyLevel.zeeplevel"));
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
    fn lists_loose_files_in_kind_directories() {
        let root = std::env::temp_dir().join("modkist-sideload-loose");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Plugins")).unwrap();
        fs::write(root.join("Plugins/LooseMod.dll"), b"dll").unwrap();
        fs::create_dir_all(root.join("Blueprints")).unwrap();
        fs::write(root.join("Blueprints/LooseLevel.zeeplevel"), b"level").unwrap();

        let entries = list_all_entries(&root).unwrap();
        assert_eq!(entries.len(), 2);

        let loose_plugin = entries
            .iter()
            .find(|entry| entry.id == "Plugins/LooseMod.dll")
            .expect("loose plugin dll should be listed");
        assert_eq!(loose_plugin.name, "LooseMod");
        assert_eq!(loose_plugin.source_type, SideloadSourceType::Dll);

        let loose_blueprint = entries
            .iter()
            .find(|entry| entry.id == "Blueprints/LooseLevel.zeeplevel")
            .expect("loose blueprint should be listed");
        assert_eq!(loose_blueprint.name, "LooseLevel");
        assert_eq!(loose_blueprint.source_type, SideloadSourceType::Zeeplevel);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_loose_files_in_root_directory() {
        let root = std::env::temp_dir().join("modkist-sideload-root-loose");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("RootMod.dll"), b"dll").unwrap();
        fs::write(root.join("RootLevel.zeeplevel"), b"level").unwrap();

        let entries = list_all_entries(&root).unwrap();
        assert_eq!(entries.len(), 2);

        let loose_plugin = entries
            .iter()
            .find(|entry| entry.id == "RootMod.dll")
            .expect("root plugin dll should be listed");
        assert_eq!(loose_plugin.name, "RootMod");
        assert_eq!(loose_plugin.target_kind, SideloadTargetKind::Plugins);
        assert_eq!(loose_plugin.source_type, SideloadSourceType::Dll);

        let loose_blueprint = entries
            .iter()
            .find(|entry| entry.id == "RootLevel.zeeplevel")
            .expect("root blueprint should be listed");
        assert_eq!(loose_blueprint.name, "RootLevel");
        assert_eq!(loose_blueprint.target_kind, SideloadTargetKind::Blueprints);
        assert_eq!(loose_blueprint.source_type, SideloadSourceType::Zeeplevel);

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
            false,
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

    #[test]
    fn classifies_loose_file_selections() {
        let root = std::env::temp_dir().join("modkist-sideload-loose-classify");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let dll_a = root.join("a.dll");
        let dll_b = root.join("b.dll");
        let level = root.join("level.zeeplevel");
        fs::write(&dll_a, b"dll").unwrap();
        fs::write(&dll_b, b"dll").unwrap();
        fs::write(&level, b"level").unwrap();

        assert_eq!(
            classify_loose_files(&[dll_a.clone(), dll_b.clone()]).unwrap(),
            ArchiveContentKind::PluginsOnly
        );
        assert_eq!(
            classify_loose_files(&[level.clone()]).unwrap(),
            ArchiveContentKind::BlueprintsOnly
        );
        assert_eq!(
            classify_loose_files(&[dll_a, level]).unwrap(),
            ArchiveContentKind::Mixed
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn names_multi_file_bundle_from_shared_parent() {
        let root = std::env::temp_dir().join("modkist-sideload-multi-name");
        let _ = fs::remove_dir_all(&root);
        let parent = root.join("CoolMod");
        fs::create_dir_all(&parent).unwrap();

        let dll_a = parent.join("a.dll");
        let dll_b = parent.join("b.dll");
        fs::write(&dll_a, b"dll").unwrap();
        fs::write(&dll_b, b"dll").unwrap();

        assert_eq!(
            folder_name_from_sources(&[dll_a, dll_b]).unwrap(),
            "CoolMod"
        );

        let other = root.join("Other");
        fs::create_dir_all(&other).unwrap();
        let dll_c = other.join("c.dll");
        let dll_d = parent.join("d.dll");
        fs::write(&dll_c, b"dll").unwrap();
        fs::write(&dll_d, b"dll").unwrap();

        assert_eq!(
            folder_name_from_sources(&[dll_c, dll_d]).unwrap(),
            "c"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn installs_multiple_loose_files_as_one_entry() {
        let root = std::env::temp_dir().join("modkist-sideload-multi-install");
        let _ = fs::remove_dir_all(&root);
        let plugins_dir = root.join("Plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        let source_dir = root.join("MyMod");
        fs::create_dir_all(&source_dir).unwrap();
        let dll_a = source_dir.join("a.dll");
        let dll_b = source_dir.join("b.dll");
        fs::write(&dll_a, b"dll-a").unwrap();
        fs::write(&dll_b, b"dll-b").unwrap();

        let entry = install_loose_files(
            &[dll_a, dll_b],
            &plugins_dir,
            "MyMod",
            SideloadTargetKind::Plugins,
            false,
        )
        .unwrap();

        assert_eq!(entry.id, "Plugins/MyMod");
        assert_eq!(entry.source_type, SideloadSourceType::Archive);
        assert!(!entry.linked);
        assert!(plugins_dir.join("MyMod/a.dll").is_file());
        assert!(plugins_dir.join("MyMod/b.dll").is_file());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn installs_multiple_loose_files_as_symlinks() {
        let root = std::env::temp_dir().join("modkist-sideload-multi-link");
        let _ = fs::remove_dir_all(&root);
        let sideload_dir = root.join("Sideloaded");
        let plugins_dir = sideload_dir.join("Plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        let source_dir = root.join("MyMod");
        fs::create_dir_all(&source_dir).unwrap();
        let dll_a = source_dir.join("a.dll");
        let dll_b = source_dir.join("b.dll");
        fs::write(&dll_a, b"dll-a").unwrap();
        fs::write(&dll_b, b"dll-b").unwrap();

        let entry = install_loose_files(
            &[dll_a.clone(), dll_b.clone()],
            &plugins_dir,
            "MyMod",
            SideloadTargetKind::Plugins,
            true,
        )
        .unwrap();

        assert_eq!(entry.id, "Plugins/MyMod");
        assert!(entry.linked);
        assert!(path_is_symlink(&plugins_dir.join("MyMod/a.dll")));
        assert!(path_is_symlink(&plugins_dir.join("MyMod/b.dll")));
        assert!(plugins_dir.join("MyMod/a.dll").is_file());
        assert!(plugins_dir.join("MyMod/b.dll").is_file());

        let entries = list_all_entries(&sideload_dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].linked);

        fs::remove_dir_all(plugins_dir.join("MyMod")).unwrap();
        assert!(dll_a.is_file());
        assert!(dll_b.is_file());
        assert!(list_all_entries(&sideload_dir).unwrap().is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_linking_zip_archives() {
        let root = std::env::temp_dir().join("modkist-sideload-reject-zip-link");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let zip_source = root.join("ArchiveMod.zip");
        fs::write(&zip_source, b"not a real zip").unwrap();

        let error = add_single_sideloaded_mod(&root, &zip_source, None, true).unwrap_err();
        assert!(
            error.contains("Zip archives must not be linked"),
            "unexpected error: {error}"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn links_arbitrary_files_with_target_choice() {
        let root = std::env::temp_dir().join("modkist-sideload-link-any");
        let _ = fs::remove_dir_all(&root);
        let plugins_dir = root.join("Plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        let source_dir = root.join("Assets");
        fs::create_dir_all(&source_dir).unwrap();
        let json = source_dir.join("config.json");
        let txt = source_dir.join("readme.txt");
        fs::write(&json, b"{}").unwrap();
        fs::write(&txt, b"hello").unwrap();

        assert_eq!(
            classify_link_files(&[json.clone(), txt.clone()]),
            ArchiveContentKind::Mixed
        );

        let needs_choice =
            add_multi_sideloaded_mod(&root, &[json.clone(), txt.clone()], None, true).unwrap();
        assert!(matches!(
            needs_choice,
            AddSideloadedModResult::NeedsTargetChoice { .. }
        ));

        let result = add_multi_sideloaded_mod(
            &root,
            &[json.clone(), txt.clone()],
            Some(SideloadTargetKind::Plugins),
            true,
        )
        .unwrap();

        let AddSideloadedModResult::Added { entry } = result else {
            panic!("expected linked entry");
        };
        assert!(entry.linked);
        assert_eq!(entry.id, "Plugins/Assets");
        assert!(path_is_symlink(&plugins_dir.join("Assets/config.json")));
        assert!(path_is_symlink(&plugins_dir.join("Assets/readme.txt")));
        assert!(json.is_file());
        assert!(txt.is_file());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn links_single_arbitrary_file_with_target() {
        let root = std::env::temp_dir().join("modkist-sideload-link-single-any");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Plugins")).unwrap();

        let source = root.join("notes.md");
        fs::write(&source, b"# notes").unwrap();

        let needs_choice = add_single_sideloaded_mod(&root, &source, None, true).unwrap();
        assert!(matches!(
            needs_choice,
            AddSideloadedModResult::NeedsTargetChoice { .. }
        ));

        let result = add_single_sideloaded_mod(
            &root,
            &source,
            Some(SideloadTargetKind::Plugins),
            true,
        )
        .unwrap();
        let AddSideloadedModResult::Added { entry } = result else {
            panic!("expected linked entry");
        };
        assert!(entry.linked);
        assert!(path_is_symlink(&root.join("Plugins/notes/notes.md")));
        assert!(source.is_file());

        let _ = fs::remove_dir_all(&root);
    }
}
