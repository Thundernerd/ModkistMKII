use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::fs_move::move_dir;
use crate::game_path::game_directory;
use crate::mod_folder::is_valid_install_folder_name;
use crate::modio_client::ModioState;

pub const PROFILES_STORE_PATH: &str = "modkist-profiles.json";
pub const VANILLA_PROFILE_ID: &str = "vanilla";
pub const USER_PROFILE_ID: &str = "user";

const PROFILES_KEY: &str = "profiles";
const ACTIVE_PROFILE_ID_KEY: &str = "activeProfileId";
const MIGRATED_KEY: &str = "migrated";
const ARCHIVES_MIGRATED_KEY: &str = "archivesMigrated";

const BEPINEX_PLUGINS: &str = "BepInEx/plugins";
const PROFILE_ARCHIVES_DIR: &str = "profiles";
const MODS_DIR: &str = "Mods";
const BLUEPRINTS_DIR: &str = "Blueprints";
const IMPORTED_PROFILE_NAME: &str = "Imported mods";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileKind {
    Vanilla,
    User,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProfile {
    id: String,
    name: String,
    kind: ProfileKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileStoreData {
    profiles: Vec<StoredProfile>,
    active_profile_id: String,
    migrated: bool,
    archives_migrated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub kind: ProfileKind,
    pub install_blocked: bool,
    pub is_active: bool,
    pub selectable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileListResult {
    pub profiles: Vec<ProfileSummary>,
    pub active_profile_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProfileInfo {
    pub id: String,
    pub name: String,
    pub kind: ProfileKind,
    pub install_blocked: bool,
}

fn is_valid_mod_folder_name(name: &str) -> bool {
    is_valid_install_folder_name(name)
}

fn bepinex_plugins_dir(game_dir: &Path) -> PathBuf {
    game_dir.join(BEPINEX_PLUGINS)
}

fn live_kind_dir(game_dir: &Path, kind_dir_name: &str) -> PathBuf {
    bepinex_plugins_dir(game_dir).join(kind_dir_name)
}

pub fn profile_archives_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve(PROFILE_ARCHIVES_DIR, BaseDirectory::AppData)
        .map_err(|error| format!("Could not resolve profile archives directory: {error}"))
}

pub fn profile_archive_root(app: &AppHandle, profile_id: &str) -> Result<PathBuf, String> {
    Ok(profile_archives_root(app)?.join(profile_id))
}

fn legacy_modkist_dir(game_dir: &Path) -> PathBuf {
    bepinex_plugins_dir(game_dir).join(".modkist")
}

fn legacy_profile_archives_root(game_dir: &Path) -> PathBuf {
    legacy_modkist_dir(game_dir).join("profiles")
}

fn profile_archive_root_at(archives_root: &Path, profile_id: &str) -> PathBuf {
    archives_root.join(profile_id)
}

fn archive_kind_dir_at(archives_root: &Path, profile_id: &str, kind_dir_name: &str) -> PathBuf {
    profile_archive_root_at(archives_root, profile_id).join(kind_dir_name)
}

fn load_store_data(app: &AppHandle) -> Result<ProfileStoreData, String> {
    let store = app.store(PROFILES_STORE_PATH).map_err(|e| e.to_string())?;
    let profiles = store
        .get(PROFILES_KEY)
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let active_profile_id = store
        .get(ACTIVE_PROFILE_ID_KEY)
        .and_then(|value| value.as_str().map(str::to_string))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| VANILLA_PROFILE_ID.to_string());
    let migrated = store
        .get(MIGRATED_KEY)
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let archives_migrated = store
        .get(ARCHIVES_MIGRATED_KEY)
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    Ok(ProfileStoreData {
        profiles,
        active_profile_id,
        migrated,
        archives_migrated,
    })
}

fn save_store_data(app: &AppHandle, data: &ProfileStoreData) -> Result<(), String> {
    let store = app.store(PROFILES_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(
        PROFILES_KEY,
        serde_json::to_value(&data.profiles).map_err(|e| e.to_string())?,
    );
    store.set(
        ACTIVE_PROFILE_ID_KEY,
        serde_json::json!(data.active_profile_id),
    );
    store.set(MIGRATED_KEY, serde_json::json!(data.migrated));
    store.set(
        ARCHIVES_MIGRATED_KEY,
        serde_json::json!(data.archives_migrated),
    );
    store.save().map_err(|e| e.to_string())
}

fn default_builtin_profiles(username: Option<&str>) -> Vec<StoredProfile> {
    vec![
        StoredProfile {
            id: VANILLA_PROFILE_ID.to_string(),
            name: "Vanilla".to_string(),
            kind: ProfileKind::Vanilla,
        },
        StoredProfile {
            id: USER_PROFILE_ID.to_string(),
            name: username
                .map(str::to_string)
                .unwrap_or_else(|| "My account".to_string()),
            kind: ProfileKind::User,
        },
    ]
}

fn ensure_builtin_profiles(data: &mut ProfileStoreData, username: Option<&str>) {
    if data.profiles.is_empty() {
        data.profiles = default_builtin_profiles(username);
        if data.active_profile_id.is_empty() {
            data.active_profile_id = VANILLA_PROFILE_ID.to_string();
        }
        return;
    }

    let has_vanilla = data
        .profiles
        .iter()
        .any(|profile| profile.id == VANILLA_PROFILE_ID);
    let has_user = data
        .profiles
        .iter()
        .any(|profile| profile.id == USER_PROFILE_ID);

    if !has_vanilla {
        data.profiles.insert(
            0,
            StoredProfile {
                id: VANILLA_PROFILE_ID.to_string(),
                name: "Vanilla".to_string(),
                kind: ProfileKind::Vanilla,
            },
        );
    }

    if !has_user {
        data.profiles.insert(
            1,
            StoredProfile {
                id: USER_PROFILE_ID.to_string(),
                name: username
                    .map(str::to_string)
                    .unwrap_or_else(|| "My account".to_string()),
                kind: ProfileKind::User,
            },
        );
    }

    if let Some(user_profile) = data
        .profiles
        .iter_mut()
        .find(|profile| profile.id == USER_PROFILE_ID)
    {
        if let Some(name) = username {
            user_profile.name = name.to_string();
        }
    }
}

fn profile_by_id<'a>(data: &'a ProfileStoreData, profile_id: &str) -> Option<&'a StoredProfile> {
    data.profiles.iter().find(|profile| profile.id == profile_id)
}

fn install_blocked_for(profile: &StoredProfile) -> bool {
    profile.kind == ProfileKind::Vanilla
}

fn to_summary(
    profile: &StoredProfile,
    active_profile_id: &str,
    logged_in: bool,
) -> ProfileSummary {
    let selectable = profile.kind != ProfileKind::User || logged_in;
    ProfileSummary {
        id: profile.id.clone(),
        name: profile.name.clone(),
        kind: profile.kind,
        install_blocked: install_blocked_for(profile),
        is_active: profile.id == active_profile_id,
        selectable,
    }
}

fn move_valid_folders(from_dir: &Path, to_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(to_dir).map_err(|e| {
        format!(
            "Could not create profile directory {}: {e}",
            to_dir.display()
        )
    })?;

    if !from_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(from_dir)
        .map_err(|e| format!("Could not read {}: {e}", from_dir.display()))?
    {
        let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Could not read entry type: {e}"))?;

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !is_valid_mod_folder_name(&name) {
                continue;
            }

            let dest = to_dir.join(entry.file_name());
            if dest.exists() {
                fs::remove_dir_all(&dest).map_err(|e| {
                    format!("Could not replace existing profile folder {}: {e}", dest.display())
                })?;
            }
            move_dir(&path, &dest).map_err(|e| {
                format!("Could not move mod folder {}: {e}", path.display())
            })?;
        }
    }

    Ok(())
}

fn clear_valid_mod_folders(kind_dir: &Path) -> Result<(), String> {
    if !kind_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(kind_dir)
        .map_err(|e| format!("Could not read {}: {e}", kind_dir.display()))?
    {
        let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
        if !entry
            .file_type()
            .map_err(|e| format!("Could not read entry type: {e}"))?
            .is_dir()
        {
            continue;
        }

        if !is_valid_mod_folder_name(&entry.file_name().to_string_lossy()) {
            continue;
        }

        fs::remove_dir_all(entry.path()).map_err(|e| {
            format!(
                "Could not clear mod folder {}: {e}",
                entry.path().display()
            )
        })?;
    }

    Ok(())
}

fn live_has_valid_mod_folders(game_dir: &Path) -> Result<bool, String> {
    for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
        let kind_dir = live_kind_dir(game_dir, kind_dir_name);
        if !kind_dir.is_dir() {
            continue;
        }

        for entry in fs::read_dir(&kind_dir)
            .map_err(|e| format!("Could not read {}: {e}", kind_dir.display()))?
        {
            let entry = entry.map_err(|e| format!("Could not read directory entry: {e}"))?;
            if entry
                .file_type()
                .map_err(|e| format!("Could not read entry type: {e}"))?
                .is_dir()
                && is_valid_mod_folder_name(&entry.file_name().to_string_lossy())
            {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn save_active_profile_at(
    game_dir: &Path,
    archives_root: &Path,
    profile_id: &str,
) -> Result<(), String> {
    log::debug!("Saving live mods to profile archive '{profile_id}'");
    for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
        let live_dir = live_kind_dir(game_dir, kind_dir_name);
        let archive_dir = archive_kind_dir_at(archives_root, profile_id, kind_dir_name);
        move_valid_folders(&live_dir, &archive_dir)?;
    }
    Ok(())
}

pub fn save_active_profile(
    app: &AppHandle,
    game_dir: &Path,
    profile_id: &str,
) -> Result<(), String> {
    save_active_profile_at(game_dir, &profile_archives_root(app)?, profile_id)
}

fn restore_profile_at(
    game_dir: &Path,
    archives_root: &Path,
    profile_id: &str,
) -> Result<(), String> {
    log::debug!("Restoring profile '{profile_id}' to live mod folders");
    for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
        let live_dir = live_kind_dir(game_dir, kind_dir_name);
        let archive_dir = archive_kind_dir_at(archives_root, profile_id, kind_dir_name);
        clear_valid_mod_folders(&live_dir)?;
        move_valid_folders(&archive_dir, &live_dir)?;
    }
    Ok(())
}

pub fn restore_profile(
    app: &AppHandle,
    game_dir: &Path,
    profile_id: &str,
) -> Result<(), String> {
    restore_profile_at(game_dir, &profile_archives_root(app)?, profile_id)
}

fn new_custom_profile_id() -> Result<String, String> {
    Ok(format!(
        "custom-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ))
}

fn adopt_live_mods_into_profile(
    app: &AppHandle,
    game_dir: &Path,
    profile_id: &str,
) -> Result<(), String> {
    let archives_root = profile_archives_root(app)?;
    for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
        let live_dir = live_kind_dir(game_dir, kind_dir_name);
        let archive_dir = archive_kind_dir_at(&archives_root, profile_id, kind_dir_name);
        move_valid_folders(&live_dir, &archive_dir)?;
    }
    restore_profile_at(game_dir, &archives_root, profile_id)
}

fn ensure_profile_archive_dirs(archives_root: &Path, profile_id: &str) -> Result<(), String> {
    for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
        let archive_dir = archive_kind_dir_at(archives_root, profile_id, kind_dir_name);
        fs::create_dir_all(&archive_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn migrate_single_profile_archive(
    legacy_root: Option<&Path>,
    archives_root: &Path,
    profile_id: &str,
) -> Result<(), String> {
    let Some(legacy_root) = legacy_root else {
        return Ok(());
    };

    let from = legacy_root.join(profile_id);
    if !from.is_dir() {
        return Ok(());
    }

    let to = archives_root.join(profile_id);
    fs::create_dir_all(archives_root).map_err(|e| e.to_string())?;
    if to.exists() {
        for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
            move_valid_folders(&from.join(kind_dir_name), &to.join(kind_dir_name))?;
        }
        fs::remove_dir_all(&from).map_err(|error| {
            format!(
                "Could not remove legacy profile archive {} after migration: {error}",
                from.display()
            )
        })?;
    } else {
        move_dir(&from, &to)?;
    }

    Ok(())
}

fn remove_legacy_modkist_folder(game_dir: &Path) -> Result<(), String> {
    let modkist_dir = legacy_modkist_dir(game_dir);
    if !modkist_dir.exists() {
        return Ok(());
    }

    fs::remove_dir_all(&modkist_dir).map_err(|error| {
        format!(
            "Could not remove legacy Modkist folder {}: {error}",
            modkist_dir.display()
        )
    })?;
    log::info!("Removed legacy folder {}", modkist_dir.display());
    Ok(())
}

fn remove_profile_archive_dirs_at(
    app_data_archive: &Path,
    legacy_archive: Option<&Path>,
) -> Result<(), String> {
    if app_data_archive.is_dir() {
        fs::remove_dir_all(app_data_archive).map_err(|error| {
            format!(
                "Could not remove profile archive {}: {error}",
                app_data_archive.display()
            )
        })?;
        log::debug!("Removed profile archive {}", app_data_archive.display());
    }

    if let Some(legacy_archive) = legacy_archive {
        if legacy_archive.is_dir() {
            fs::remove_dir_all(legacy_archive).map_err(|error| {
                format!(
                    "Could not remove legacy profile archive {}: {error}",
                    legacy_archive.display()
                )
            })?;
            log::debug!("Removed legacy profile archive {}", legacy_archive.display());
        }
    }

    Ok(())
}

fn remove_profile_archive_from_disk(app: &AppHandle, profile_id: &str) -> Result<(), String> {
    let app_data_archive = profile_archive_root(app, profile_id)?;
    let legacy_archive = game_directory(app)
        .ok()
        .map(|game_dir| legacy_profile_archives_root(&game_dir).join(profile_id));
    remove_profile_archive_dirs_at(&app_data_archive, legacy_archive.as_deref())
}

fn migrate_profile_archives_to_app_data(
    app: &AppHandle,
    data: &mut ProfileStoreData,
) -> Result<(), String> {
    if data.archives_migrated {
        return Ok(());
    }

    let archives_root = profile_archives_root(app)?;
    fs::create_dir_all(&archives_root).map_err(|e| e.to_string())?;

    let legacy_root = game_directory(app)
        .ok()
        .map(|game_dir| legacy_profile_archives_root(&game_dir));

    let mut profile_ids: HashSet<String> = data
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();

    if let Some(ref legacy_root) = legacy_root {
        if legacy_root.is_dir() {
            if let Ok(entries) = fs::read_dir(legacy_root) {
                for entry in entries.flatten() {
                    if entry
                        .file_type()
                        .map(|file_type| file_type.is_dir())
                        .unwrap_or(false)
                    {
                        profile_ids.insert(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    for profile_id in profile_ids {
        migrate_single_profile_archive(legacy_root.as_deref(), &archives_root, &profile_id)?;
        ensure_profile_archive_dirs(&archives_root, &profile_id)?;
    }

    if let Some(game_dir) = game_directory(app).ok() {
        remove_legacy_modkist_folder(&game_dir)?;
    }

    data.archives_migrated = true;
    log::info!(
        "Moved profile archives to {}",
        archives_root.display()
    );
    Ok(())
}

fn run_migration_if_needed(app: &AppHandle, data: &mut ProfileStoreData) -> Result<(), String> {
    if data.migrated {
        return Ok(());
    }

    let game_dir = match game_directory(app) {
        Ok(game_dir) => game_dir,
        Err(_) => return Ok(()),
    };

    if live_has_valid_mod_folders(&game_dir)? {
        let profile_id = new_custom_profile_id()?;
        data.profiles.push(StoredProfile {
            id: profile_id.clone(),
            name: IMPORTED_PROFILE_NAME.to_string(),
            kind: ProfileKind::Custom,
        });
        adopt_live_mods_into_profile(app, &game_dir, &profile_id)?;
        data.active_profile_id = profile_id.clone();
        log::info!(
            "Imported existing mods into new profile '{IMPORTED_PROFILE_NAME}' ({profile_id})"
        );
    } else {
        data.active_profile_id = VANILLA_PROFILE_ID.to_string();
        restore_profile(app, &game_dir, VANILLA_PROFILE_ID)?;
    }

    data.migrated = true;
    save_store_data(app, data)
}

fn prepare_store(app: &AppHandle, modio_state: &ModioState) -> Result<ProfileStoreData, String> {
    let auth = modio_state.auth_status();
    let username = auth.username.as_deref();
    let mut data = load_store_data(app)?;
    ensure_builtin_profiles(&mut data, username);
    run_migration_if_needed(app, &mut data)?;
    let mut data = load_store_data(app)?;
    ensure_builtin_profiles(&mut data, username);
    migrate_profile_archives_to_app_data(app, &mut data)?;
    if let Ok(game_dir) = game_directory(app) {
        remove_legacy_modkist_folder(&game_dir)?;
    }
    save_store_data(app, &data)?;
    Ok(data)
}

pub fn active_profile_info(app: &AppHandle, modio_state: &ModioState) -> Result<ActiveProfileInfo, String> {
    let data = prepare_store(app, modio_state)?;
    let profile = profile_by_id(&data, &data.active_profile_id).ok_or_else(|| {
        format!(
            "Active profile {} is not configured.",
            data.active_profile_id
        )
    })?;

    Ok(ActiveProfileInfo {
        id: profile.id.clone(),
        name: profile.name.clone(),
        kind: profile.kind,
        install_blocked: install_blocked_for(profile),
    })
}

pub fn active_profile_install_blocked(
    app: &AppHandle,
    modio_state: &ModioState,
) -> Result<bool, String> {
    Ok(active_profile_info(app, modio_state)?.install_blocked)
}

pub fn active_profile_is_user(app: &AppHandle, modio_state: &ModioState) -> Result<bool, String> {
    Ok(active_profile_info(app, modio_state)?.kind == ProfileKind::User)
}

pub fn logout_requires_profile_selection(
    app: &AppHandle,
    modio_state: &ModioState,
) -> Result<bool, String> {
    if !modio_state.auth_status().logged_in {
        return Ok(false);
    }
    Ok(active_profile_info(app, modio_state)?.kind == ProfileKind::User)
}

fn switch_profile_inner(
    app: &AppHandle,
    modio_state: &ModioState,
    target_profile_id: &str,
) -> Result<ActiveProfileInfo, String> {
    let mut data = prepare_store(app, modio_state)?;
    if data.active_profile_id == target_profile_id {
        log::debug!("Profile '{target_profile_id}' is already active");
        return active_profile_info(app, modio_state);
    }

    let target = profile_by_id(&data, target_profile_id)
        .ok_or_else(|| format!("Profile {target_profile_id} does not exist."))?
        .clone();

    if target.kind == ProfileKind::User && !modio_state.auth_status().logged_in {
        return Err("Sign in to use your account profile.".into());
    }

    modio_state.cancel_subscription_sync();

    let from_profile_id = data.active_profile_id.clone();
    log::info!(
        "Switching profile: {} -> {} ({})",
        from_profile_id,
        target_profile_id,
        target.name
    );

    let game_dir = game_directory(app).ok();

    if let Some(game_dir) = &game_dir {
        save_active_profile(app, game_dir, &from_profile_id)?;
    }

    data.active_profile_id = target_profile_id.to_string();
    save_store_data(app, &data)?;

    match game_dir {
        Some(game_dir) => restore_profile(app, &game_dir, target_profile_id)?,
        None => log::debug!(
            "Profile switch stored without mod folder changes: game directory is not configured"
        ),
    }
    // mod.io metadata is global per mod id; keep the API cache across profile
    // switches so browse/install flows can reuse it when returning to a profile.

    log::info!("Active profile is now '{}'", target.name);
    Ok(ActiveProfileInfo {
        id: target.id.clone(),
        name: target.name.clone(),
        kind: target.kind,
        install_blocked: install_blocked_for(&target),
    })
}

#[tauri::command]
pub fn list_profiles(
    app: AppHandle,
    modio_state: tauri::State<'_, ModioState>,
) -> Result<ProfileListResult, String> {
    let logged_in = modio_state.auth_status().logged_in;
    let data = prepare_store(&app, &modio_state)?;
    let profiles = data
        .profiles
        .iter()
        .map(|profile| to_summary(profile, &data.active_profile_id, logged_in))
        .collect();

    Ok(ProfileListResult {
        profiles,
        active_profile_id: data.active_profile_id,
    })
}

#[tauri::command]
pub fn get_active_profile(
    app: AppHandle,
    modio_state: tauri::State<'_, ModioState>,
) -> Result<ActiveProfileInfo, String> {
    active_profile_info(&app, &modio_state)
}

#[tauri::command]
pub fn switch_profile(
    app: AppHandle,
    modio_state: tauri::State<'_, ModioState>,
    profile_id: String,
) -> Result<ActiveProfileInfo, String> {
    switch_profile_inner(&app, &modio_state, profile_id.trim())
}

#[tauri::command]
pub fn create_profile(
    app: AppHandle,
    modio_state: tauri::State<'_, ModioState>,
    name: String,
) -> Result<ProfileSummary, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Enter a profile name.".into());
    }

    let mut data = prepare_store(&app, &modio_state)?;
    if data.profiles.iter().any(|profile| profile.name == trimmed) {
        return Err("A profile with this name already exists.".into());
    }

    let id = new_custom_profile_id()?;
    let profile = StoredProfile {
        id: id.clone(),
        name: trimmed.to_string(),
        kind: ProfileKind::Custom,
    };
    data.profiles.push(profile.clone());
    save_store_data(&app, &data)?;
    log::info!("Created profile '{}' ({id})", trimmed);

    if let Ok(archives_root) = profile_archives_root(&app) {
        ensure_profile_archive_dirs(&archives_root, &id)?;
    }

    let logged_in = modio_state.auth_status().logged_in;
    Ok(to_summary(&profile, &data.active_profile_id, logged_in))
}

#[tauri::command]
pub fn delete_profile(
    app: AppHandle,
    modio_state: tauri::State<'_, ModioState>,
    profile_id: String,
) -> Result<(), String> {
    let profile_id = profile_id.trim();
    let data = prepare_store(&app, &modio_state)?;
    let profile = profile_by_id(&data, profile_id)
        .ok_or_else(|| format!("Profile {profile_id} does not exist."))?;

    if profile.kind != ProfileKind::Custom {
        return Err("Built-in profiles cannot be deleted.".into());
    }

    if data.active_profile_id == profile_id {
        return Err("Switch to another profile before deleting this one.".into());
    }

    let profile_name = profile.name.clone();
    remove_profile_archive_from_disk(&app, profile_id)?;

    let mut data = data;
    data.profiles.retain(|entry| entry.id != profile_id);
    save_store_data(&app, &data)?;
    log::info!("Deleted profile '{profile_name}' ({profile_id})");

    Ok(())
}

#[tauri::command]
pub fn rename_profile(
    app: AppHandle,
    modio_state: tauri::State<'_, ModioState>,
    profile_id: String,
    name: String,
) -> Result<ProfileSummary, String> {
    let profile_id = profile_id.trim();
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Enter a profile name.".into());
    }

    let mut data = prepare_store(&app, &modio_state)?;
    let profile = data
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| format!("Profile {profile_id} does not exist."))?;

    if profile.kind != ProfileKind::Custom {
        return Err("Built-in profiles cannot be renamed.".into());
    }

    if data
        .profiles
        .iter()
        .any(|entry| entry.id != profile_id && entry.name == trimmed)
    {
        return Err("A profile with this name already exists.".into());
    }

    let profile = data
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id)
        .expect("profile exists");
    profile.name = trimmed.to_string();
    let profile = profile.clone();
    save_store_data(&app, &data)?;

    let logged_in = modio_state.auth_status().logged_in;
    Ok(to_summary(&profile, &data.active_profile_id, logged_in))
}

#[tauri::command]
pub fn logout_requires_profile_selection_command(
    app: AppHandle,
    modio_state: tauri::State<'_, ModioState>,
) -> Result<bool, String> {
    logout_requires_profile_selection(&app, &modio_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_game_and_archives() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let game_dir = temp.path().join("game");
        let archives_root = temp.path().join("appdata").join(PROFILE_ARCHIVES_DIR);
        fs::create_dir_all(game_dir.join("BepInEx/plugins/Mods")).unwrap();
        fs::create_dir_all(game_dir.join("BepInEx/plugins/Blueprints")).unwrap();
        (temp, game_dir, archives_root)
    }

    fn write_mod_folder(game_dir: &Path, kind: &str, folder_name: &str) {
        let path = game_dir
            .join(BEPINEX_PLUGINS)
            .join(kind)
            .join(folder_name);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("mod.dll"), b"test").unwrap();
    }

    #[test]
    fn save_and_restore_round_trip_moves_folders() {
        let (_temp, game_dir, archives_root) = temp_game_and_archives();
        write_mod_folder(&game_dir, MODS_DIR, "10_20");
        write_mod_folder(&game_dir, BLUEPRINTS_DIR, "30_40");

        save_active_profile_at(&game_dir, &archives_root, "test-profile").unwrap();
        assert!(!live_kind_dir(&game_dir, MODS_DIR).join("10_20").exists());
        assert!(archive_kind_dir_at(&archives_root, "test-profile", MODS_DIR)
            .join("10_20")
            .exists());

        restore_profile_at(&game_dir, &archives_root, "test-profile").unwrap();
        assert!(live_kind_dir(&game_dir, MODS_DIR).join("10_20").exists());
        assert!(live_kind_dir(&game_dir, BLUEPRINTS_DIR)
            .join("30_40")
            .exists());
    }

    #[test]
    fn switch_clears_live_before_restore() {
        let (_temp, game_dir, archives_root) = temp_game_and_archives();
        write_mod_folder(&game_dir, MODS_DIR, "1_1");
        save_active_profile_at(&game_dir, &archives_root, "profile-a").unwrap();

        write_mod_folder(&game_dir, MODS_DIR, "2_2");
        save_active_profile_at(&game_dir, &archives_root, "profile-b").unwrap();
        assert!(archive_kind_dir_at(&archives_root, "profile-b", MODS_DIR)
            .join("2_2")
            .exists());

        restore_profile_at(&game_dir, &archives_root, "profile-a").unwrap();
        assert!(live_kind_dir(&game_dir, MODS_DIR).join("1_1").exists());
        assert!(!live_kind_dir(&game_dir, MODS_DIR).join("2_2").exists());
    }

    #[test]
    fn adopt_live_mods_into_profile_keeps_mods_active() {
        let (_temp, game_dir, archives_root) = temp_game_and_archives();
        write_mod_folder(&game_dir, MODS_DIR, "10_20");
        write_mod_folder(&game_dir, BLUEPRINTS_DIR, "30_40");

        for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
            let live_dir = live_kind_dir(&game_dir, kind_dir_name);
            let archive_dir = archive_kind_dir_at(&archives_root, "imported-profile", kind_dir_name);
            move_valid_folders(&live_dir, &archive_dir).unwrap();
        }
        restore_profile_at(&game_dir, &archives_root, "imported-profile").unwrap();

        assert!(live_kind_dir(&game_dir, MODS_DIR).join("10_20").exists());
        assert!(live_kind_dir(&game_dir, BLUEPRINTS_DIR)
            .join("30_40")
            .exists());
    }

    #[test]
    fn profile_operations_leave_unmanaged_entries_alone() {
        let (_temp, game_dir, archives_root) = temp_game_and_archives();
        write_mod_folder(&game_dir, MODS_DIR, "10_20");
        let manual_mod_dir = live_kind_dir(&game_dir, MODS_DIR).join("MyManualMod");
        fs::create_dir_all(&manual_mod_dir).unwrap();
        fs::write(manual_mod_dir.join("mod.dll"), b"manual").unwrap();
        let loose_file = live_kind_dir(&game_dir, MODS_DIR).join("readme.txt");
        fs::write(&loose_file, b"keep me").unwrap();

        for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
            let live_dir = live_kind_dir(&game_dir, kind_dir_name);
            let archive_dir = archive_kind_dir_at(&archives_root, "imported-profile", kind_dir_name);
            move_valid_folders(&live_dir, &archive_dir).unwrap();
        }
        restore_profile_at(&game_dir, &archives_root, "imported-profile").unwrap();
        assert!(manual_mod_dir.exists());
        assert!(loose_file.exists());

        save_active_profile_at(&game_dir, &archives_root, "imported-profile").unwrap();
        restore_profile_at(&game_dir, &archives_root, VANILLA_PROFILE_ID).unwrap();
        assert!(manual_mod_dir.exists());
        assert!(loose_file.exists());
        assert!(!live_kind_dir(&game_dir, MODS_DIR).join("10_20").exists());
    }

    #[test]
    fn migrate_profile_archives_moves_legacy_folders() {
        let temp = tempfile::tempdir().expect("tempdir");
        let game_dir = temp.path().join("game");
        let legacy_root = legacy_profile_archives_root(&game_dir);
        let archives_root = temp.path().join("appdata").join(PROFILE_ARCHIVES_DIR);
        let legacy_mod_dir = legacy_root.join("profile-a").join(MODS_DIR).join("10_20");
        fs::create_dir_all(&legacy_mod_dir).unwrap();
        fs::write(legacy_mod_dir.join("mod.dll"), b"test").unwrap();

        migrate_single_profile_archive(Some(&legacy_root), &archives_root, "profile-a").unwrap();
        ensure_profile_archive_dirs(&archives_root, "profile-a").unwrap();
        remove_legacy_modkist_folder(&game_dir).unwrap();

        assert!(!legacy_modkist_dir(&game_dir).exists());
        assert!(archive_kind_dir_at(&archives_root, "profile-a", MODS_DIR)
            .join("10_20")
            .exists());
    }

    #[test]
    fn remove_profile_archive_from_disk_removes_app_data_and_legacy_folders() {
        let temp = tempfile::tempdir().expect("tempdir");
        let game_dir = temp.path().join("game");
        let archives_root = temp.path().join("appdata").join(PROFILE_ARCHIVES_DIR);
        let profile_id = "custom-123";
        let app_data_archive = archives_root.join(profile_id);
        let legacy_archive = legacy_profile_archives_root(&game_dir).join(profile_id);
        fs::create_dir_all(app_data_archive.join(MODS_DIR).join("10_20")).unwrap();
        fs::create_dir_all(legacy_archive.join(MODS_DIR).join("20_30")).unwrap();

        remove_profile_archive_dirs_at(&app_data_archive, Some(&legacy_archive)).unwrap();

        assert!(!app_data_archive.exists());
        assert!(!legacy_archive.exists());
    }

    #[test]
    fn builtin_profiles_are_not_deletable() {
        let vanilla = StoredProfile {
            id: VANILLA_PROFILE_ID.to_string(),
            name: "Vanilla".to_string(),
            kind: ProfileKind::Vanilla,
        };
        assert!(install_blocked_for(&vanilla));
        assert_ne!(vanilla.kind, ProfileKind::Custom);
    }
}
