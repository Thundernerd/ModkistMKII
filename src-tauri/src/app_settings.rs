use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const SETTINGS_STORE_PATH: &str = "modkist-settings.json";
const AUTO_UPDATE_MODS_KEY: &str = "autoUpdateMods";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub auto_update_mods: bool,
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
    AppSettings {
        auto_update_mods: auto_update_mods_enabled(&app),
    }
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
    Ok(AppSettings {
        auto_update_mods: enabled,
    })
}
