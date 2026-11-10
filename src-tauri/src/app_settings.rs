use std::path::PathBuf;

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const SETTINGS_STORE_PATH: &str = "modkist-settings.json";
const AUTO_UPDATE_MODS_KEY: &str = "autoUpdateMods";
const SKIP_SIGN_IN_KEY: &str = "skipSignIn";
const IGNORE_BEPINEX_VERSION_WARNING_KEY: &str = "ignoreBepInExVersionWarning";
const CUSTOM_WINE_PREFIX_PATH_KEY: &str = "customWinePrefixPath";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub auto_update_mods: bool,
    pub skip_sign_in: bool,
    pub ignore_bepinex_version_warning: bool,
    pub custom_wine_prefix_path: Option<String>,
}

fn read_skip_sign_in(app: &AppHandle) -> bool {
    app.store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(SKIP_SIGN_IN_KEY))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn read_ignore_bepinex_version_warning(app: &AppHandle) -> bool {
    app.store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(IGNORE_BEPINEX_VERSION_WARNING_KEY))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

pub fn ignore_bepinex_version_warning_enabled(app: &AppHandle) -> bool {
    read_ignore_bepinex_version_warning(app)
}

fn read_custom_wine_prefix_path(app: &AppHandle) -> Option<String> {
    app.store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(CUSTOM_WINE_PREFIX_PATH_KEY))
        .and_then(|value| value.as_str().map(str::to_string))
        .filter(|path| !path.trim().is_empty())
}

/// The Wine prefix folder the user chose in Settings, if any. It wins over detection.
pub fn custom_wine_prefix_path(app: &AppHandle) -> Option<PathBuf> {
    read_custom_wine_prefix_path(app).map(PathBuf::from)
}

fn app_settings_for(app: &AppHandle) -> AppSettings {
    AppSettings {
        auto_update_mods: auto_update_mods_enabled(app),
        skip_sign_in: read_skip_sign_in(app),
        ignore_bepinex_version_warning: read_ignore_bepinex_version_warning(app),
        custom_wine_prefix_path: read_custom_wine_prefix_path(app),
    }
}

pub fn clear_skip_sign_in(app: &AppHandle) -> Result<(), String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    let _ = store.delete(SKIP_SIGN_IN_KEY);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn auto_update_mods_enabled(app: &AppHandle) -> bool {
    app.store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(AUTO_UPDATE_MODS_KEY))
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

#[tauri::command]
pub fn get_app_settings(app: AppHandle) -> AppSettings {
    app_settings_for(&app)
}

#[tauri::command]
pub fn get_ignore_bepinex_version_warning_enabled(app: AppHandle) -> bool {
    ignore_bepinex_version_warning_enabled(&app)
}

#[tauri::command]
pub fn set_auto_update_mods(app: AppHandle, enabled: bool) -> Result<AppSettings, String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(AUTO_UPDATE_MODS_KEY, serde_json::json!(enabled));
    store.save().map_err(|e| e.to_string())?;
    log::info!(
        "Auto-update mods {}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(app_settings_for(&app))
}

#[tauri::command]
pub fn set_custom_wine_prefix_path(app: AppHandle, path: String) -> Result<AppSettings, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Choose a Wine prefix folder.".into());
    }
    crate::wine_prefix::validate_wine_prefix(std::path::Path::new(path))?;
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(CUSTOM_WINE_PREFIX_PATH_KEY, serde_json::json!(path));
    store.save().map_err(|e| e.to_string())?;
    log::info!("Custom Wine prefix set to {path}");
    Ok(app_settings_for(&app))
}

#[tauri::command]
pub fn clear_custom_wine_prefix_path(app: AppHandle) -> Result<AppSettings, String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    let _ = store.delete(CUSTOM_WINE_PREFIX_PATH_KEY);
    store.save().map_err(|e| e.to_string())?;
    log::info!("Custom Wine prefix cleared, using auto-detection");
    Ok(app_settings_for(&app))
}

#[tauri::command]
pub fn set_ignore_bepinex_version_warning(
    app: AppHandle,
    enabled: bool,
) -> Result<AppSettings, String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(
        IGNORE_BEPINEX_VERSION_WARNING_KEY,
        serde_json::json!(enabled),
    );
    store.save().map_err(|e| e.to_string())?;
    log::info!(
        "BepInEx version warning {}",
        if enabled { "suppressed" } else { "enabled" }
    );
    Ok(app_settings_for(&app))
}

#[tauri::command]
pub fn remember_ignore_bepinex_version_warning(app: AppHandle) -> Result<AppSettings, String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(IGNORE_BEPINEX_VERSION_WARNING_KEY, serde_json::json!(true));
    store.save().map_err(|e| e.to_string())?;
    log::info!("BepInEx version warning suppressed for future launches");
    Ok(app_settings_for(&app))
}

#[tauri::command]
pub fn remember_skip_sign_in(app: AppHandle) -> Result<AppSettings, String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(SKIP_SIGN_IN_KEY, serde_json::json!(true));
    store.save().map_err(|e| e.to_string())?;
    log::info!("Sign-in prompt skipped for future launches");
    Ok(app_settings_for(&app))
}
