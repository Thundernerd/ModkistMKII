use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::game_path::game_directory;
use crate::mod_folder::is_valid_install_folder_name;
use crate::modio_client::ModioState;

pub const PROFILES_STORE_PATH: &str = "modkist-profiles.json";
pub const VANILLA_PROFILE_ID: &str = "vanilla";
pub const USER_PROFILE_ID: &str = "user";

const PROFILES_KEY: &str = "profiles";
const ACTIVE_PROFILE_ID_KEY: &str = "activeProfileId";
const MIGRATED_KEY: &str = "migrated";

const BEPINEX_PLUGINS: &str = "BepInEx/plugins";
const MODKIST_ROOT: &str = ".modkist/profiles";
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

pub fn profile_archive_root(game_dir: &Path, profile_id: &str) -> PathBuf {
    bepinex_plugins_dir(game_dir)
        .join(MODKIST_ROOT)
        .join(profile_id)
}

fn archive_kind_dir(game_dir: &Path, profile_id: &str, kind_dir_name: &str) -> PathBuf {
    profile_archive_root(game_dir, profile_id).join(kind_dir_name)
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

    Ok(ProfileStoreData {
        profiles,
        active_profile_id,
        migrated,
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
            fs::rename(&path, &dest).map_err(|e| {
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

pub fn save_active_profile(game_dir: &Path, profile_id: &str) -> Result<(), String> {
    log::debug!("Saving live mods to profile archive '{profile_id}'");
    for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
        let live_dir = live_kind_dir(game_dir, kind_dir_name);
        let archive_dir = archive_kind_dir(game_dir, profile_id, kind_dir_name);
        move_valid_folders(&live_dir, &archive_dir)?;
    }
    Ok(())
}

pub fn restore_profile(game_dir: &Path, profile_id: &str) -> Result<(), String> {
    log::debug!("Restoring profile '{profile_id}' to live mod folders");
    for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
        let live_dir = live_kind_dir(game_dir, kind_dir_name);
        let archive_dir = archive_kind_dir(game_dir, profile_id, kind_dir_name);
        clear_valid_mod_folders(&live_dir)?;
        move_valid_folders(&archive_dir, &live_dir)?;
    }
    Ok(())
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

fn adopt_live_mods_into_profile(game_dir: &Path, profile_id: &str) -> Result<(), String> {
    for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
        let live_dir = live_kind_dir(game_dir, kind_dir_name);
        let archive_dir = archive_kind_dir(game_dir, profile_id, kind_dir_name);
        move_valid_folders(&live_dir, &archive_dir)?;
    }
    restore_profile(game_dir, profile_id)
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
        adopt_live_mods_into_profile(&game_dir, &profile_id)?;
        data.active_profile_id = profile_id.clone();
        log::info!(
            "Imported existing mods into new profile '{IMPORTED_PROFILE_NAME}' ({profile_id})"
        );
    } else {
        data.active_profile_id = VANILLA_PROFILE_ID.to_string();
        restore_profile(&game_dir, VANILLA_PROFILE_ID)?;
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
        save_active_profile(game_dir, &from_profile_id)?;
    }

    data.active_profile_id = target_profile_id.to_string();
    save_store_data(app, &data)?;

    match game_dir {
        Some(game_dir) => restore_profile(&game_dir, target_profile_id)?,
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

    if let Ok(game_dir) = game_directory(&app) {
        for kind_dir_name in [MODS_DIR, BLUEPRINTS_DIR] {
            let archive_dir = archive_kind_dir(&game_dir, &id, kind_dir_name);
            fs::create_dir_all(&archive_dir).map_err(|e| e.to_string())?;
        }
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
    let mut data = data;
    data.profiles.retain(|entry| entry.id != profile_id);
    save_store_data(&app, &data)?;
    log::info!("Deleted profile '{profile_name}' ({profile_id})");

    if let Ok(game_dir) = game_directory(&app) {
        let archive_root = profile_archive_root(&game_dir, profile_id);
        if archive_root.exists() {
            fs::remove_dir_all(&archive_root).map_err(|e| {
                format!(
                    "Could not remove profile archive {}: {e}",
                    archive_root.display()
                )
            })?;
        }
    }

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

    fn temp_game_dir() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let game_dir = temp.path().join("game");
        fs::create_dir_all(game_dir.join("BepInEx/plugins/Mods")).unwrap();
        fs::create_dir_all(game_dir.join("BepInEx/plugins/Blueprints")).unwrap();
        (temp, game_dir)
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
        let (_temp, game_dir) = temp_game_dir();
        write_mod_folder(&game_dir, MODS_DIR, "10_20");
        write_mod_folder(&game_dir, BLUEPRINTS_DIR, "30_40");

        save_active_profile(&game_dir, "test-profile").unwrap();
        assert!(!live_kind_dir(&game_dir, MODS_DIR).join("10_20").exists());
        assert!(archive_kind_dir(&game_dir, "test-profile", MODS_DIR)
            .join("10_20")
            .exists());

        restore_profile(&game_dir, "test-profile").unwrap();
        assert!(live_kind_dir(&game_dir, MODS_DIR).join("10_20").exists());
        assert!(live_kind_dir(&game_dir, BLUEPRINTS_DIR)
            .join("30_40")
            .exists());
    }

    #[test]
    fn switch_clears_live_before_restore() {
        let (_temp, game_dir) = temp_game_dir();
        write_mod_folder(&game_dir, MODS_DIR, "1_1");
        save_active_profile(&game_dir, "profile-a").unwrap();

        write_mod_folder(&game_dir, MODS_DIR, "2_2");
        save_active_profile(&game_dir, "profile-b").unwrap();
        assert!(archive_kind_dir(&game_dir, "profile-b", MODS_DIR)
            .join("2_2")
            .exists());

        restore_profile(&game_dir, "profile-a").unwrap();
        assert!(live_kind_dir(&game_dir, MODS_DIR).join("1_1").exists());
        assert!(!live_kind_dir(&game_dir, MODS_DIR).join("2_2").exists());
    }

    #[test]
    fn adopt_live_mods_into_profile_keeps_mods_active() {
        let (_temp, game_dir) = temp_game_dir();
        write_mod_folder(&game_dir, MODS_DIR, "10_20");
        write_mod_folder(&game_dir, BLUEPRINTS_DIR, "30_40");

        adopt_live_mods_into_profile(&game_dir, "imported-profile").unwrap();

        assert!(live_kind_dir(&game_dir, MODS_DIR).join("10_20").exists());
        assert!(live_kind_dir(&game_dir, BLUEPRINTS_DIR)
            .join("30_40")
            .exists());
    }

    #[test]
    fn profile_operations_leave_unmanaged_entries_alone() {
        let (_temp, game_dir) = temp_game_dir();
        write_mod_folder(&game_dir, MODS_DIR, "10_20");
        let manual_mod_dir = live_kind_dir(&game_dir, MODS_DIR).join("MyManualMod");
        fs::create_dir_all(&manual_mod_dir).unwrap();
        fs::write(manual_mod_dir.join("mod.dll"), b"manual").unwrap();
        let loose_file = live_kind_dir(&game_dir, MODS_DIR).join("readme.txt");
        fs::write(&loose_file, b"keep me").unwrap();

        adopt_live_mods_into_profile(&game_dir, "imported-profile").unwrap();
        assert!(manual_mod_dir.exists());
        assert!(loose_file.exists());

        save_active_profile(&game_dir, "imported-profile").unwrap();
        restore_profile(&game_dir, "vanilla").unwrap();
        assert!(manual_mod_dir.exists());
        assert!(loose_file.exists());
        assert!(!live_kind_dir(&game_dir, MODS_DIR).join("10_20").exists());
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
