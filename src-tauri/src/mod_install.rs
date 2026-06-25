use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use modio::types::id::Id;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::bepinex::has_bepinex_structure;
use crate::game_path::game_directory;
use crate::mod_download::download_modfile;
use crate::modio_client::{
    fetch_subscribed_mod_ids, format_modio_error, is_mod_unavailable, subscribe_to_mod,
    with_rate_limit_retry, ModioState,
};
use crate::zip_extract::{install_downloaded_mod, sanitize_filename};

const BEPINEX_PLUGINS: &str = "BepInEx/plugins";
const MODS_DIR: &str = "Mods";
const BLUEPRINTS_DIR: &str = "Blueprints";
const PLUGIN_TAG: &str = "Plugin";
const BLUEPRINT_TAG: &str = "Blueprint";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstalledModKind {
    Plugin,
    Blueprint,
}

impl InstalledModKind {
    fn directory_name(self) -> &'static str {
        match self {
            Self::Plugin => MODS_DIR,
            Self::Blueprint => BLUEPRINTS_DIR,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModRecord {
    pub mod_id: u64,
    pub file_id: u64,
    pub kind: InstalledModKind,
    pub folder_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallBlocker {
    pub mod_id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInstallState {
    pub status: String,
    pub installed_file_id: Option<u64>,
    pub latest_file_id: Option<u64>,
    pub kind: Option<InstalledModKind>,
    pub can_uninstall: bool,
    pub uninstall_blocked_by: Vec<UninstallBlocker>,
}

#[derive(Debug, Clone)]
enum InstallStatus {
    NotInstalled,
    UpToDate { file_id: u64 },
    UpdateAvailable {
        installed_file_id: u64,
        latest_file_id: u64,
    },
}

impl InstallStatus {
    fn into_response(
        self,
        kind: Option<InstalledModKind>,
        blockers: Vec<UninstallBlocker>,
    ) -> ModInstallState {
        let is_installed = !matches!(self, Self::NotInstalled);
        let can_uninstall = is_installed && blockers.is_empty();

        match self {
            Self::NotInstalled => ModInstallState {
                status: "notInstalled".into(),
                installed_file_id: None,
                latest_file_id: None,
                kind,
                can_uninstall: false,
                uninstall_blocked_by: blockers,
            },
            Self::UpToDate { file_id } => ModInstallState {
                status: "upToDate".into(),
                installed_file_id: Some(file_id),
                latest_file_id: Some(file_id),
                kind,
                can_uninstall,
                uninstall_blocked_by: blockers,
            },
            Self::UpdateAvailable {
                installed_file_id,
                latest_file_id,
            } => ModInstallState {
                status: "updateAvailable".into(),
                installed_file_id: Some(installed_file_id),
                latest_file_id: Some(latest_file_id),
                kind,
                can_uninstall,
                uninstall_blocked_by: blockers,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModEntry {
    pub mod_id: u64,
    pub file_id: u64,
    pub kind: InstalledModKind,
    pub folder_name: String,
    pub name: String,
    pub summary: String,
    pub logo_url: String,
    pub tags: Vec<String>,
    pub update_available: bool,
    pub latest_file_id: Option<u64>,
    pub can_uninstall: bool,
    pub uninstall_blocked_by: Vec<UninstallBlocker>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallModResult {
    pub installed: Vec<u64>,
    pub skipped: Vec<u64>,
}

fn bepinex_plugins_dir(game_dir: &Path) -> PathBuf {
    game_dir.join(BEPINEX_PLUGINS)
}

fn kind_root_dir(game_dir: &Path, kind: InstalledModKind) -> PathBuf {
    bepinex_plugins_dir(game_dir).join(kind.directory_name())
}

fn folder_name(mod_id: u64, file_id: u64) -> String {
    format!("{mod_id}_{file_id}")
}

fn parse_folder_name(name: &str) -> Option<(u64, u64)> {
    let (mod_part, file_part) = name.split_once('_')?;
    Some((mod_part.parse().ok()?, file_part.parse().ok()?))
}

fn is_valid_install_folder_name(name: &str) -> bool {
    parse_folder_name(name).is_some()
}

fn remove_install_directory_entry(path: &Path, is_dir: bool) -> Result<(), String> {
    if is_dir {
        fs::remove_dir_all(path).map_err(|e| {
            format!("Could not remove invalid mod folder {}: {e}", path.display())
        })
    } else {
        fs::remove_file(path).map_err(|e| {
            format!("Could not remove invalid mod file {}: {e}", path.display())
        })
    }
}

fn remove_invalid_install_entries(game_dir: &Path) -> Result<(), String> {
    for kind in [InstalledModKind::Plugin, InstalledModKind::Blueprint] {
        let kind_dir = kind_root_dir(game_dir, kind);
        if !kind_dir.is_dir() {
            continue;
        }

        for entry in
            fs::read_dir(&kind_dir).map_err(|e| format!("Could not read {}: {e}", kind_dir.display()))?
        {
            let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("Could not read entry type: {e}"))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();

            let should_remove = if file_type.is_dir() {
                !is_valid_install_folder_name(&name)
            } else {
                true
            };

            if should_remove {
                remove_install_directory_entry(&path, file_type.is_dir())?;
            }
        }
    }

    Ok(())
}

fn mod_kind_from_tags(tags: &[String]) -> Result<InstalledModKind, String> {
    let has_plugin = tags.iter().any(|tag| tag == PLUGIN_TAG);
    let has_blueprint = tags.iter().any(|tag| tag == BLUEPRINT_TAG);

    // When both tags are present, prefer Plugin (matches browse filter semantics).
    if has_plugin {
        return Ok(InstalledModKind::Plugin);
    }
    if has_blueprint {
        return Ok(InstalledModKind::Blueprint);
    }

    Err("Mod is not tagged Plugin or Blueprint.".into())
}

fn ensure_install_prerequisites(game_dir: &Path) -> Result<(), String> {
    if !has_bepinex_structure(game_dir) {
        return Err("BepInEx is not installed in your game directory.".into());
    }
    Ok(())
}

fn scan_kind_directory(kind_dir: &Path, kind: InstalledModKind) -> Result<Vec<InstalledModRecord>, String> {
    if !kind_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in fs::read_dir(kind_dir).map_err(|e| format!("Could not read {}: {e}", kind_dir.display()))? {
        let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Could not read entry type: {e}"))?;
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some((mod_id, file_id)) = parse_folder_name(&name) else {
            continue;
        };

        records.push(InstalledModRecord {
            mod_id,
            file_id,
            kind,
            folder_name: name.into_owned(),
        });
    }

    Ok(records)
}

fn scan_installed_mods(game_dir: &Path) -> Result<Vec<InstalledModRecord>, String> {
    let mut records = scan_kind_directory(&kind_root_dir(game_dir, InstalledModKind::Plugin), InstalledModKind::Plugin)?;
    records.extend(scan_kind_directory(
        &kind_root_dir(game_dir, InstalledModKind::Blueprint),
        InstalledModKind::Blueprint,
    )?);
    Ok(records)
}

fn find_installed_record(records: &[InstalledModRecord], mod_id: u64) -> Option<&InstalledModRecord> {
    records.iter().find(|record| record.mod_id == mod_id)
}

fn remove_installed_record_folder(game_dir: &Path, record: &InstalledModRecord) -> Result<(), String> {
    let path = kind_root_dir(game_dir, record.kind).join(&record.folder_name);
    if path.is_dir() {
        fs::remove_dir_all(&path).map_err(|e| {
            format!("Could not remove installed mod folder {}: {e}", path.display())
        })?;
    }
    Ok(())
}

fn remove_installed_mod_folders(game_dir: &Path, mod_id: u64) -> Result<(), String> {
    for kind in [InstalledModKind::Plugin, InstalledModKind::Blueprint] {
        let kind_dir = kind_root_dir(game_dir, kind);
        if !kind_dir.is_dir() {
            continue;
        }

        for entry in fs::read_dir(&kind_dir).map_err(|e| format!("Could not read {}: {e}", kind_dir.display()))? {
            let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some((entry_mod_id, _)) = parse_folder_name(&name) else {
                continue;
            };
            if entry_mod_id != mod_id {
                continue;
            }

            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path).map_err(|e| {
                    format!("Could not remove installed mod folder {}: {e}", path.display())
                })?;
            }
        }
    }

    Ok(())
}

enum ModFetchOutcome {
    Found(modio::types::mods::Mod),
    Unavailable,
    Failed(String),
}

async fn fetch_mod_outcome(state: &ModioState, mod_id: u64) -> ModFetchOutcome {
    if state.cached_mod_unavailable(mod_id) {
        return ModFetchOutcome::Unavailable;
    }

    let game_id = match state.game_id() {
        Ok(game_id) => game_id,
        Err(message) => return ModFetchOutcome::Failed(message),
    };
    let client = match state.get_mods_client() {
        Ok(client) => client,
        Err(message) => return ModFetchOutcome::Failed(message),
    };

    let outcome = match client
        .get_mod(Id::new(game_id), Id::new(mod_id))
        .await
    {
        Ok(response) => match response.data().await {
            Ok(mod_) => ModFetchOutcome::Found(mod_),
            Err(error) => ModFetchOutcome::Failed(error.to_string()),
        },
        Err(error) => {
            if is_mod_unavailable(&error) {
                ModFetchOutcome::Unavailable
            } else if error.is_ratelimited() {
                tokio::time::sleep(std::time::Duration::from_secs(61)).await;
                return Box::pin(fetch_mod_outcome(state, mod_id)).await;
            } else {
                ModFetchOutcome::Failed(format_modio_error(error))
            }
        }
    };

    match &outcome {
        ModFetchOutcome::Unavailable => state.mark_mod_unavailable(mod_id),
        _ => {}
    }

    outcome
}

async fn prepare_installed_records(
    state: &ModioState,
    game_dir: &Path,
) -> Result<(Vec<InstalledModRecord>, HashMap<u64, modio::types::mods::Mod>), String> {
    remove_invalid_install_entries(game_dir)?;
    let records = scan_installed_mods(game_dir)?;
    let mut available = Vec::with_capacity(records.len());
    let mut mods_by_id = HashMap::new();

    for record in records {
        match fetch_mod_outcome(state, record.mod_id).await {
            ModFetchOutcome::Found(mod_) => {
                mods_by_id.insert(record.mod_id, mod_);
                available.push(record);
            }
            ModFetchOutcome::Unavailable => {
                remove_installed_record_folder(game_dir, &record)?;
                state.invalidate_mod_cache(record.mod_id);
            }
            ModFetchOutcome::Failed(_) => available.push(record),
        }
    }

    Ok((available, mods_by_id))
}

async fn scan_installed_mods_after_cleanup(
    state: &ModioState,
    game_dir: &Path,
) -> Result<Vec<InstalledModRecord>, String> {
    Ok(prepare_installed_records(state, game_dir).await?.0)
}

async fn refresh_dependency_map(
    state: &ModioState,
    dependency_map: &mut HashMap<u64, Vec<u64>>,
    records: &[InstalledModRecord],
) -> Result<(), String> {
    for record in records {
        if dependency_map.contains_key(&record.mod_id) {
            continue;
        }
        dependency_map.insert(
            record.mod_id,
            fetch_dependency_ids(state, record.mod_id).await?,
        );
    }
    Ok(())
}

async fn fetch_mod(
    state: &ModioState,
    mod_id: u64,
) -> Result<modio::types::mods::Mod, String> {
    match fetch_mod_outcome(state, mod_id).await {
        ModFetchOutcome::Found(mod_) => Ok(mod_),
        ModFetchOutcome::Unavailable => {
            Err(format!("Mod {mod_id} is no longer available on mod.io."))
        }
        ModFetchOutcome::Failed(message) => Err(message),
    }
}

async fn fetch_dependency_ids(state: &ModioState, mod_id: u64) -> Result<Vec<u64>, String> {
    if let Some(cached) = state.cached_dependencies(mod_id) {
        return Ok(cached);
    }

    let dependencies: Vec<u64> = with_rate_limit_retry(|| async {
        let game_id = state.game_id()?;
        let client = state.get_mods_client()?;
        let response = client
            .get_mod_dependencies(Id::new(game_id), Id::new(mod_id))
            .await
            .map_err(format_modio_error)?;
        let list = response.data().await.map_err(|e| e.to_string())?;
        Ok(list.data.into_iter().map(|dep| dep.mod_id.get()).collect())
    })
    .await?;

    state.store_dependencies(mod_id, dependencies.clone());
    Ok(dependencies)
}

async fn collect_install_order(state: &ModioState, root_mod_id: u64) -> Result<Vec<u64>, String> {
    let mut nodes = HashSet::new();
    let mut stack = vec![root_mod_id];
    let mut dep_map: HashMap<u64, Vec<u64>> = HashMap::new();

    while let Some(mod_id) = stack.pop() {
        if !nodes.insert(mod_id) {
            continue;
        }

        let deps = fetch_dependency_ids(state, mod_id).await?;
        dep_map.insert(mod_id, deps.clone());
        stack.extend(deps);
    }

    let mut in_degree: HashMap<u64, usize> = HashMap::new();
    let mut dependents: HashMap<u64, Vec<u64>> = HashMap::new();

    for &mod_id in &nodes {
        let deps = dep_map.get(&mod_id).cloned().unwrap_or_default();
        let relevant_deps: Vec<u64> = deps.into_iter().filter(|dep| nodes.contains(dep)).collect();
        in_degree.insert(mod_id, relevant_deps.len());
        for dep in relevant_deps {
            dependents.entry(dep).or_default().push(mod_id);
        }
    }

    let mut queue: Vec<u64> = in_degree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(&mod_id, _)| mod_id)
        .collect();
    let mut order = Vec::with_capacity(nodes.len());

    while let Some(mod_id) = queue.pop() {
        order.push(mod_id);
        if let Some(children) = dependents.get(&mod_id) {
            for child in children {
                let degree = in_degree.get_mut(child).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    queue.push(*child);
                }
            }
        }
    }

    if order.len() != nodes.len() {
        return Err("Circular dependency detected.".into());
    }

    Ok(order)
}

async fn build_dependency_map(
    state: &ModioState,
    installed_ids: &HashSet<u64>,
) -> Result<HashMap<u64, Vec<u64>>, String> {
    let mut map = HashMap::with_capacity(installed_ids.len());
    for &mod_id in installed_ids {
        map.insert(mod_id, fetch_dependency_ids(state, mod_id).await?);
    }
    Ok(map)
}

fn installed_dependents(
    mod_id: u64,
    dependency_map: &HashMap<u64, Vec<u64>>,
    installed_ids: &HashSet<u64>,
) -> Vec<u64> {
    installed_ids
        .iter()
        .copied()
        .filter(|installed_id| {
            *installed_id != mod_id
                && dependency_map
                    .get(installed_id)
                    .is_some_and(|deps| deps.contains(&mod_id))
        })
        .collect()
}

async fn uninstall_blockers_for(
    state: &ModioState,
    mod_id: u64,
    installed_ids: &HashSet<u64>,
    dependency_map: &HashMap<u64, Vec<u64>>,
) -> Result<Vec<UninstallBlocker>, String> {
    let mut blockers = Vec::new();

    for dependent_id in installed_dependents(mod_id, dependency_map, installed_ids) {
        let mod_ = fetch_mod(state, dependent_id).await?;
        blockers.push(UninstallBlocker {
            mod_id: dependent_id,
            name: mod_.name,
        });
    }

    blockers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(blockers)
}

fn format_uninstall_blocked_error(blockers: &[UninstallBlocker]) -> String {
    let names: Vec<&str> = blockers.iter().map(|blocker| blocker.name.as_str()).collect();
    match names.as_slice() {
        [name] => format!("Cannot uninstall: required by {name}."),
        _ => format!("Cannot uninstall: required by {}.", names.join(", ")),
    }
}

async fn latest_file_id(state: &ModioState, mod_id: u64) -> Result<u64, String> {
    let mod_ = fetch_mod(state, mod_id).await?;
    let file = mod_
        .modfile
        .as_ref()
        .ok_or_else(|| format!("Mod {mod_id} has no downloadable file."))?;
    Ok(file.id.get())
}

async fn install_state_for_mod(
    state: &ModioState,
    mod_id: u64,
    records: &[InstalledModRecord],
    dependency_map: &HashMap<u64, Vec<u64>>,
) -> Result<ModInstallState, String> {
    let installed_ids: HashSet<u64> = records.iter().map(|record| record.mod_id).collect();
    let installed = find_installed_record(records, mod_id);
    let latest_file_id = latest_file_id(state, mod_id).await.ok();

    let status = match (installed, latest_file_id) {
        (None, _) => InstallStatus::NotInstalled,
        (Some(record), Some(latest)) if record.file_id == latest => InstallStatus::UpToDate {
            file_id: latest,
        },
        (Some(record), Some(latest)) => InstallStatus::UpdateAvailable {
            installed_file_id: record.file_id,
            latest_file_id: latest,
        },
        (Some(record), None) => InstallStatus::UpToDate {
            file_id: record.file_id,
        },
    };

    let blockers = if matches!(status, InstallStatus::NotInstalled) {
        Vec::new()
    } else {
        uninstall_blockers_for(state, mod_id, &installed_ids, dependency_map).await?
    };

    Ok(status.into_response(
        installed.map(|record| record.kind),
        blockers,
    ))
}

async fn install_single_mod(
    state: &ModioState,
    game_dir: &Path,
    mod_id: u64,
) -> Result<(), String> {
    let mod_ = fetch_mod(state, mod_id).await?;
    let file = mod_
        .modfile
        .as_ref()
        .ok_or_else(|| format!("Mod {mod_id} has no downloadable file."))?;
    let file_id = file.id.get();
    let filename = file.filename.clone();
    let expected_size = file.filesize;
    let download_url = file.download.binary_url.to_string();
    let tags: Vec<String> = mod_.tags.into_iter().map(|tag| tag.name).collect();
    let kind = mod_kind_from_tags(&tags)?;

    remove_installed_mod_folders(game_dir, mod_id)?;

    let target_dir = kind_root_dir(game_dir, kind).join(folder_name(mod_id, file_id));
    fs::create_dir_all(&target_dir).map_err(|e| {
        format!(
            "Could not create mod install directory {}: {e}",
            target_dir.display()
        )
    })?;

    let temp_dir = std::env::temp_dir().join("modkist-mod-install");
    fs::create_dir_all(&temp_dir).map_err(|e| format!("Could not create temp directory: {e}"))?;
    let download_path = temp_dir.join(format!("{mod_id}_{file_id}_{}", sanitize_filename(&filename)));

    download_modfile(
        state,
        &download_url,
        &download_path,
        Some(expected_size),
    )
    .await?;

    install_downloaded_mod(&download_path, &target_dir, &filename)?;
    let _ = fs::remove_file(&download_path);
    state.invalidate_mod_cache(mod_id);

    Ok(())
}

async fn install_mod_internal(
    state: &ModioState,
    game_dir: &Path,
    mod_id: u64,
) -> Result<InstallModResult, String> {
    let order = collect_install_order(state, mod_id).await?;
    let mut records = scan_installed_mods_after_cleanup(state, game_dir).await?;
    let mut dependency_map = HashMap::new();
    refresh_dependency_map(state, &mut dependency_map, &records).await?;

    let mut installed = Vec::new();
    let mut skipped = Vec::new();

    for target_mod_id in order {
        if state.auth_status().logged_in {
            subscribe_to_mod(state, target_mod_id).await?;
        }

        let state_for_mod =
            install_state_for_mod(state, target_mod_id, &records, &dependency_map).await?;
        if matches!(state_for_mod.status.as_str(), "upToDate") {
            skipped.push(target_mod_id);
            continue;
        }

        install_single_mod(state, game_dir, target_mod_id).await?;
        installed.push(target_mod_id);
        records = scan_installed_mods(game_dir)?;
        refresh_dependency_map(state, &mut dependency_map, &records).await?;
    }

    Ok(InstallModResult { installed, skipped })
}

#[tauri::command]
pub async fn sync_subscribed_mods(
    app: AppHandle,
    state: State<'_, ModioState>,
) -> Result<InstallModResult, String> {
    sync_subscribed_mods_inner(&app, &state).await
}

async fn sync_subscribed_mods_inner(
    app: &AppHandle,
    state: &ModioState,
) -> Result<InstallModResult, String> {
    if !state.auth_status().logged_in {
        return Ok(InstallModResult {
            installed: Vec::new(),
            skipped: Vec::new(),
        });
    }

    let game_dir = game_directory(app)?;
    ensure_install_prerequisites(&game_dir)?;
    let mod_ids = fetch_subscribed_mod_ids(state).await?;

    let mut installed = Vec::new();
    let mut skipped = Vec::new();

    for mod_id in mod_ids {
        let result = install_mod_internal(state, &game_dir, mod_id).await?;
        installed.extend(result.installed);
        skipped.extend(result.skipped);
    }

    installed.sort_unstable();
    installed.dedup();
    skipped.sort_unstable();
    skipped.dedup();

    Ok(InstallModResult { installed, skipped })
}

async fn refresh_installed_mods_inner(
    app: &AppHandle,
    state: &ModioState,
    sync_subscriptions: bool,
) -> Result<Vec<InstalledModEntry>, String> {
    if sync_subscriptions && state.auth_status().logged_in {
        sync_subscribed_mods_inner(app, state).await?;
    }
    list_installed_mods_inner(app, state).await
}

#[tauri::command]
pub async fn refresh_installed_mods(
    app: AppHandle,
    state: State<'_, ModioState>,
    sync_subscriptions: Option<bool>,
) -> Result<Vec<InstalledModEntry>, String> {
    refresh_installed_mods_inner(&app, &state, sync_subscriptions.unwrap_or(false)).await
}

#[tauri::command]
pub async fn list_installed_mods(
    app: AppHandle,
    state: State<'_, ModioState>,
) -> Result<Vec<InstalledModEntry>, String> {
    list_installed_mods_inner(&app, &state).await
}

async fn list_installed_mods_inner(
    app: &AppHandle,
    state: &ModioState,
) -> Result<Vec<InstalledModEntry>, String> {
    let game_dir = game_directory(app)?;
    ensure_install_prerequisites(&game_dir)?;

    let (records, mods_by_id) = prepare_installed_records(state, &game_dir).await?;
    let installed_ids: HashSet<u64> = records.iter().map(|record| record.mod_id).collect();
    let dependency_map = build_dependency_map(state, &installed_ids).await?;
    let mut entries = Vec::with_capacity(records.len());

    for record in records {
        let Some(mod_) = mods_by_id.get(&record.mod_id) else {
            continue;
        };

        let latest_file_id = mod_.modfile.as_ref().map(|file| file.id.get());
        let update_available = latest_file_id.is_some_and(|latest| latest != record.file_id);
        let tags: Vec<String> = mod_.tags.iter().map(|tag| tag.name.clone()).collect();
        let blockers =
            uninstall_blockers_for(state, record.mod_id, &installed_ids, &dependency_map)
                .await?;

        entries.push(InstalledModEntry {
            mod_id: record.mod_id,
            file_id: record.file_id,
            kind: record.kind,
            folder_name: record.folder_name,
            name: mod_.name.clone(),
            summary: mod_.summary.clone(),
            logo_url: mod_.logo.thumb_320x180.to_string(),
            tags,
            update_available,
            latest_file_id,
            can_uninstall: blockers.is_empty(),
            uninstall_blocked_by: blockers,
        });
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(entries)
}

#[tauri::command]
pub async fn get_mod_install_state(
    app: AppHandle,
    state: State<'_, ModioState>,
    mod_id: u64,
) -> Result<ModInstallState, String> {
    let game_dir = game_directory(&app)?;
    ensure_install_prerequisites(&game_dir)?;
    let records = scan_installed_mods_after_cleanup(&state, &game_dir).await?;
    let installed_ids: HashSet<u64> = records.iter().map(|record| record.mod_id).collect();
    let dependency_map = build_dependency_map(&state, &installed_ids).await?;
    install_state_for_mod(&state, mod_id, &records, &dependency_map).await
}

#[tauri::command]
pub async fn install_mod(
    app: AppHandle,
    state: State<'_, ModioState>,
    mod_id: u64,
) -> Result<InstallModResult, String> {
    let game_dir = game_directory(&app)?;
    ensure_install_prerequisites(&game_dir)?;
    install_mod_internal(&state, &game_dir, mod_id).await
}

#[tauri::command]
pub async fn uninstall_mod(
    app: AppHandle,
    state: State<'_, ModioState>,
    mod_id: u64,
) -> Result<(), String> {
    let game_dir = game_directory(&app)?;
    ensure_install_prerequisites(&game_dir)?;

    let records = scan_installed_mods_after_cleanup(&state, &game_dir).await?;
    let installed_ids: HashSet<u64> = records.iter().map(|record| record.mod_id).collect();

    if !installed_ids.contains(&mod_id) {
        return Err("Mod is not installed.".into());
    }

    let dependency_map = build_dependency_map(&state, &installed_ids).await?;
    let blockers = uninstall_blockers_for(&state, mod_id, &installed_ids, &dependency_map).await?;
    if !blockers.is_empty() {
        return Err(format_uninstall_blocked_error(&blockers));
    }

    remove_installed_mod_folders(&game_dir, mod_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_folder_name() {
        assert_eq!(parse_folder_name("12345_67890"), Some((12345, 67890)));
        assert_eq!(parse_folder_name("invalid"), None);
    }

    #[test]
    fn finds_installed_dependents() {
        let mut dependency_map = HashMap::new();
        dependency_map.insert(10, vec![1, 2]);
        dependency_map.insert(20, vec![3]);
        dependency_map.insert(30, vec![1]);

        let installed_ids: HashSet<u64> = [10, 20, 30].into_iter().collect();
        let mut dependents = installed_dependents(1, &dependency_map, &installed_ids);
        dependents.sort_unstable();

        assert_eq!(dependents, vec![10, 30]);
    }

    #[test]
    fn prefers_plugin_when_both_tags_present() {
        let tags = vec!["Plugin".into(), "Blueprint".into()];
        assert_eq!(mod_kind_from_tags(&tags).unwrap(), InstalledModKind::Plugin);
    }

    #[test]
    fn rejects_malformed_folder_names() {
        assert!(!is_valid_install_folder_name("invalid"));
        assert!(!is_valid_install_folder_name("12345"));
        assert!(!is_valid_install_folder_name("12345_"));
        assert!(!is_valid_install_folder_name("_67890"));
        assert!(!is_valid_install_folder_name("12345_abc"));
        assert!(is_valid_install_folder_name("12345_67890"));
    }

    #[test]
    fn removes_invalid_install_entries() {
        let root = std::env::temp_dir().join("modkist-mod-install-invalid");
        let _ = fs::remove_dir_all(&root);
        let game_dir = root.join("game");
        let mods_dir = kind_root_dir(&game_dir, InstalledModKind::Plugin);
        fs::create_dir_all(mods_dir.join("12345_67890")).unwrap();
        fs::create_dir_all(mods_dir.join("bad-folder")).unwrap();
        fs::write(mods_dir.join("loose.dll"), b"test").unwrap();

        remove_invalid_install_entries(&game_dir).unwrap();

        assert!(mods_dir.join("12345_67890").is_dir());
        assert!(!mods_dir.join("bad-folder").exists());
        assert!(!mods_dir.join("loose.dll").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn removes_installed_record_folder_by_name() {
        let root = std::env::temp_dir().join("modkist-mod-install-cleanup");
        let _ = fs::remove_dir_all(&root);
        let game_dir = root.join("game");
        let folder = kind_root_dir(&game_dir, InstalledModKind::Plugin).join("12345_67890");
        fs::create_dir_all(&folder).unwrap();

        let record = InstalledModRecord {
            mod_id: 12345,
            file_id: 67890,
            kind: InstalledModKind::Plugin,
            folder_name: "12345_67890".into(),
        };

        remove_installed_record_folder(&game_dir, &record).unwrap();
        assert!(!folder.exists());

        let _ = fs::remove_dir_all(&root);
    }
}
