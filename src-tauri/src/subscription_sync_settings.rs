use serde::Serialize;
use std::collections::HashSet;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::app_settings::SETTINGS_STORE_PATH;

const FAILED_SYNC_MODS_KEY: &str = "failedSyncModIds";
const IGNORED_SYNC_MODS_KEY: &str = "ignoredSyncModIds";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedSyncModEntry {
    pub mod_id: u64,
    pub ignored: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedSyncModList {
    pub mods: Vec<FailedSyncModEntry>,
}

fn read_u64_list(app: &AppHandle, key: &str) -> Vec<u64> {
    app.store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn write_u64_list(app: &AppHandle, key: &str, mod_ids: &[u64]) -> Result<(), String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(key, serde_json::json!(mod_ids));
    store.save().map_err(|e| e.to_string())
}

fn sort_dedup(mod_ids: &mut Vec<u64>) {
    mod_ids.sort_unstable();
    mod_ids.dedup();
}

pub fn read_failed_sync_mod_ids(app: &AppHandle) -> Vec<u64> {
    read_u64_list(app, FAILED_SYNC_MODS_KEY)
}

pub fn read_ignored_sync_mod_ids(app: &AppHandle) -> Vec<u64> {
    read_u64_list(app, IGNORED_SYNC_MODS_KEY)
}

pub fn record_failed_sync_mod(app: &AppHandle, mod_id: u64) -> Result<(), String> {
    let mut failed = read_failed_sync_mod_ids(app);
    if failed.binary_search(&mod_id).is_ok() {
        return Ok(());
    }
    failed.push(mod_id);
    sort_dedup(&mut failed);
    write_u64_list(app, FAILED_SYNC_MODS_KEY, &failed)?;
    log::info!("Recorded failed subscription sync for mod {mod_id}");
    Ok(())
}

pub fn clear_failed_sync_mod(app: &AppHandle, mod_id: u64) -> Result<(), String> {
    let mut failed = read_failed_sync_mod_ids(app);
    let original_len = failed.len();
    failed.retain(|id| *id != mod_id);
    if failed.len() == original_len {
        return Ok(());
    }
    write_u64_list(app, FAILED_SYNC_MODS_KEY, &failed)?;
    log::info!("Cleared failed subscription sync entry for mod {mod_id}");
    Ok(())
}

pub fn set_sync_mod_ignored(app: &AppHandle, mod_id: u64, ignored: bool) -> Result<(), String> {
    let mut ignored_mods = read_ignored_sync_mod_ids(app);
    let position = ignored_mods.binary_search(&mod_id);

    if ignored {
        if position.is_ok() {
            return Ok(());
        }
        ignored_mods.push(mod_id);
        sort_dedup(&mut ignored_mods);
        write_u64_list(app, IGNORED_SYNC_MODS_KEY, &ignored_mods)?;
        log::info!("Ignoring mod {mod_id} during subscription sync");
        return Ok(());
    }

    if position.is_err() {
        return Ok(());
    }
    ignored_mods.remove(position.unwrap());
    write_u64_list(app, IGNORED_SYNC_MODS_KEY, &ignored_mods)?;
    log::info!("Stopped ignoring mod {mod_id} during subscription sync");
    Ok(())
}

pub fn remove_sync_mod_tracking(app: &AppHandle, mod_id: u64) -> Result<(), String> {
    clear_failed_sync_mod(app, mod_id)?;
    set_sync_mod_ignored(app, mod_id, false)?;
    Ok(())
}

pub fn list_failed_sync_mods(app: &AppHandle) -> FailedSyncModList {
    let failed = read_failed_sync_mod_ids(app);
    let ignored = read_ignored_sync_mod_ids(app);
    let mut mod_ids = failed;
    for mod_id in ignored {
        if mod_ids.binary_search(&mod_id).is_err() {
            mod_ids.push(mod_id);
        }
    }
    sort_dedup(&mut mod_ids);

    let ignored_set: HashSet<u64> = read_ignored_sync_mod_ids(app).into_iter().collect();

    FailedSyncModList {
        mods: mod_ids
            .into_iter()
            .map(|mod_id| FailedSyncModEntry {
                mod_id,
                ignored: ignored_set.contains(&mod_id),
            })
            .collect(),
    }
}

#[tauri::command]
pub fn list_failed_sync_mods_command(app: AppHandle) -> FailedSyncModList {
    list_failed_sync_mods(&app)
}

#[tauri::command]
pub fn set_failed_sync_mod_ignored(
    app: AppHandle,
    mod_id: u64,
    ignored: bool,
) -> Result<FailedSyncModList, String> {
    set_sync_mod_ignored(&app, mod_id, ignored)?;
    Ok(list_failed_sync_mods(&app))
}

#[tauri::command]
pub async fn unsubscribe_failed_sync_mod(
    app: AppHandle,
    state: tauri::State<'_, crate::modio_client::ModioState>,
    mod_id: u64,
) -> Result<FailedSyncModList, String> {
    unsubscribe_failed_sync_mod_inner(&app, &state, mod_id).await
}

async fn unsubscribe_failed_sync_mod_inner(
    app: &AppHandle,
    state: &crate::modio_client::ModioState,
    mod_id: u64,
) -> Result<FailedSyncModList, String> {
    if let Err(error) = crate::modio_client::unsubscribe_from_mod(state, mod_id).await {
        log::warn!("Unsubscribe failed for mod {mod_id}: {error}");
        return Err(error);
    }
    remove_sync_mod_tracking(app, mod_id)?;
    Ok(list_failed_sync_mods(app))
}
