use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const SETTINGS_STORE_PATH: &str = "modkist-settings.json";
const AUTO_UPDATE_MODS_KEY: &str = "autoUpdateMods";
const SKIP_SIGN_IN_KEY: &str = "skipSignIn";
const IGNORE_BEPINEX_VERSION_WARNING_KEY: &str = "ignoreBepInExVersionWarning";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub auto_update_mods: bool,
    pub skip_sign_in: bool,
    pub ignore_bepinex_version_warning: bool,
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

fn app_settings_for(app: &AppHandle) -> AppSettings {
    AppSettings {
        auto_update_mods: auto_update_mods_enabled(app),
        skip_sign_in: read_skip_sign_in(app),
        ignore_bepinex_version_warning: read_ignore_bepinex_version_warning(app),
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
pub fn remember_skip_sign_in(app: AppHandle) -> Result<AppSettings, String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(SKIP_SIGN_IN_KEY, serde_json::json!(true));
    store.save().map_err(|e| e.to_string())?;
    log::info!("Sign-in prompt skipped for future launches");
    Ok(app_settings_for(&app))
}
