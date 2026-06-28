use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::app_settings::SETTINGS_STORE_PATH;

const LEGACY_FAILED_SYNC_MODS_KEY: &str = "failedSyncModIds";
const FAILED_SYNC_MODS_KEY: &str = "failedSyncMods";
const IGNORED_SYNC_MODS_KEY: &str = "ignoredSyncModIds";
const MAX_ERROR_DETAIL_LEN: usize = 240;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FailedSyncModRecord {
    mod_id: u64,
    error_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedSyncModEntry {
    pub mod_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_name: Option<String>,
    pub ignored: bool,
    pub error_type: String,
    pub error_detail: Option<String>,
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

pub fn read_ignored_sync_mod_ids(app: &AppHandle) -> Vec<u64> {
    read_u64_list(app, IGNORED_SYNC_MODS_KEY)
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

fn truncate_error_detail(message: &str) -> Option<String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() <= MAX_ERROR_DETAIL_LEN {
        return Some(trimmed.to_string());
    }
    Some(format!(
        "{}…",
        trimmed.chars().take(MAX_ERROR_DETAIL_LEN).collect::<String>()
    ))
}

fn dependency_failure_detail(message: &str) -> Option<String> {
    let lower = message.to_ascii_lowercase();
    if lower.contains("rate limit") {
        return Some(
            "Could not fetch required dependencies due to mod.io rate limiting.".to_string(),
        );
    }
    if lower.contains("no longer available") {
        return truncate_error_detail(message);
    }
    if lower.contains("could not be found")
        || lower.contains("private")
        || lower.contains("not subscribed")
        || lower.contains("sign in")
        || lower.contains("not logged in")
        || lower.contains("authentication")
    {
        return Some(
            "A required dependency is private or could not be loaded on mod.io.".to_string(),
        );
    }
    truncate_error_detail(message)
}

fn extract_dependency_mod_id(message: &str) -> Option<u64> {
    let rest = message.strip_prefix("Required dependency (mod ")?;
    rest.split(')').next()?.parse().ok()
}

fn merge_dependency_failure_details(
    existing: Option<&str>,
    category: &str,
    message: &str,
) -> (String, Option<String>) {
    let (error_type, new_detail) = refine_sync_failure(category, message);
    if error_type != "dependency" {
        return (error_type, new_detail);
    }

    let mut mod_ids: BTreeSet<u64> = BTreeSet::new();
    if let Some(id) = extract_dependency_mod_id(message) {
        mod_ids.insert(id);
    }
    if let Some(existing) = existing {
        if let Some(id) = extract_dependency_mod_id(existing) {
            mod_ids.insert(id);
        } else if existing.starts_with("Required dependencies could not be installed (mods ") {
            let ids = existing
                .trim_start_matches("Required dependencies could not be installed (mods ")
                .trim_end_matches(").");
            for id in ids.split(", ") {
                if let Ok(parsed) = id.parse::<u64>() {
                    mod_ids.insert(parsed);
                }
            }
        }
    }

    if mod_ids.is_empty() {
        return (error_type, new_detail.or_else(|| existing.map(str::to_string)));
    }

    let ids = mod_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    (
        error_type,
        Some(format!(
            "Required dependencies could not be installed (mods {ids})."
        )),
    )
}

fn refine_sync_failure(category: &str, message: &str) -> (String, Option<String>) {
    if category == "install_order" || category == "dependency" {
        return ("dependency".to_string(), dependency_failure_detail(message));
    }

    let lower = message.to_ascii_lowercase();
    let detail = truncate_error_detail(message);

    if lower.contains("rate limit") {
        return ("rate_limit".to_string(), detail);
    }
    if lower.contains("no longer available") {
        return ("unavailable".to_string(), detail);
    }
    if lower.contains("sign in")
        || lower.contains("not logged in")
        || lower.contains("authentication")
        || lower.contains("private")
        || lower.contains("could not be found")
        || lower.contains("not subscribed")
    {
        return ("auth".to_string(), detail);
    }
    if lower.contains("zeepkist is running") {
        return ("game_running".to_string(), detail);
    }
    if lower.contains("vanilla profile") || lower.contains("installing mods is disabled") {
        return ("profile_blocked".to_string(), detail);
    }

    (category.to_string(), detail)
}

pub fn read_failed_sync_mod_ids(app: &AppHandle) -> Vec<u64> {
    read_failed_sync_records(app)
        .into_iter()
        .map(|record| record.mod_id)
        .collect()
}

pub fn count_dependency_sync_failures(app: &AppHandle) -> u32 {
    read_failed_sync_records(app)
        .iter()
        .filter(|record| record.error_type == "dependency")
        .count() as u32
}

pub fn is_dependency_sync_failure(app: &AppHandle, mod_id: u64) -> bool {
    read_failed_sync_records(app)
        .iter()
        .any(|record| record.mod_id == mod_id && record.error_type == "dependency")
}

fn read_failed_sync_records(app: &AppHandle) -> Vec<FailedSyncModRecord> {
    let store = app.store(SETTINGS_STORE_PATH).ok();
    let Some(store) = store else {
        return Vec::new();
    };

    if let Some(value) = store.get(FAILED_SYNC_MODS_KEY) {
        if let Ok(records) = serde_json::from_value::<Vec<FailedSyncModRecord>>(value) {
            return records;
        }
    }

    read_u64_list(app, LEGACY_FAILED_SYNC_MODS_KEY)
        .into_iter()
        .map(|mod_id| FailedSyncModRecord {
            mod_id,
            error_type: "unknown".to_string(),
            error_detail: None,
        })
        .collect()
}

fn write_failed_sync_records(app: &AppHandle, records: &[FailedSyncModRecord]) -> Result<(), String> {
    let store = app.store(SETTINGS_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(FAILED_SYNC_MODS_KEY, serde_json::json!(records));
    let _ = store.delete(LEGACY_FAILED_SYNC_MODS_KEY);
    store.save().map_err(|e| e.to_string())
}

pub fn record_failed_sync_mod(
    app: &AppHandle,
    mod_id: u64,
    category: &str,
    message: &str,
) -> Result<(), String> {
    let (error_type, error_detail) = refine_sync_failure(category, message);
    let mut records = read_failed_sync_records(app);

    if let Some(existing) = records.iter_mut().find(|record| record.mod_id == mod_id) {
        let (merged_type, merged_detail) = if existing.error_type == "dependency"
            && error_type == "dependency"
        {
            merge_dependency_failure_details(existing.error_detail.as_deref(), category, message)
        } else {
            (error_type, error_detail)
        };
        existing.error_type = merged_type;
        existing.error_detail = merged_detail;
    } else {
        records.push(FailedSyncModRecord {
            mod_id,
            error_type,
            error_detail,
        });
        records.sort_unstable_by_key(|record| record.mod_id);
    }

    write_failed_sync_records(app, &records)?;
    log::info!("Recorded failed subscription sync for mod {mod_id} ({category})");
    Ok(())
}

pub fn clear_failed_sync_mod(app: &AppHandle, mod_id: u64) -> Result<(), String> {
    let mut records = read_failed_sync_records(app);
    let original_len = records.len();
    records.retain(|record| record.mod_id != mod_id);
    if records.len() == original_len {
        return Ok(());
    }
    write_failed_sync_records(app, &records)?;
    log::info!("Cleared failed subscription sync entry for mod {mod_id}");
    Ok(())
}

pub fn set_sync_mod_ignored(app: &AppHandle, mod_id: u64, ignored: bool) -> Result<(), String> {
    let mut ignored_mods = read_ignored_sync_mod_ids(app);
    let position = ignored_mods.binary_search(&mod_id);

    if ignored {
        let failed = read_failed_sync_mod_ids(app);
        if failed.binary_search(&mod_id).is_err() {
            return Err(format!(
                "Mod {mod_id} is not in the sync failures list."
            ));
        }
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

fn build_failed_sync_mod_list(
    records: &[FailedSyncModRecord],
    ignored: &[u64],
) -> FailedSyncModList {
    let ignored_set: HashSet<u64> = ignored.iter().copied().collect();

    FailedSyncModList {
        mods: records
            .iter()
            .map(|record| FailedSyncModEntry {
                mod_id: record.mod_id,
                mod_name: None,
                ignored: ignored_set.contains(&record.mod_id),
                error_type: record.error_type.clone(),
                error_detail: record.error_detail.clone(),
            })
            .collect(),
    }
}

fn build_failed_sync_mod_list_for_app(app: &AppHandle) -> FailedSyncModList {
    build_failed_sync_mod_list(
        &read_failed_sync_records(app),
        &read_ignored_sync_mod_ids(app),
    )
}

async fn enrich_failed_sync_mod_names(
    state: &crate::modio_client::ModioState,
    list: &mut FailedSyncModList,
) {
    for entry in &mut list.mods {
        entry.mod_name = crate::modio_client::resolve_mod_name(state, entry.mod_id).await;
    }
}

pub async fn list_failed_sync_mods(
    app: &AppHandle,
    state: &crate::modio_client::ModioState,
) -> FailedSyncModList {
    let mut list = build_failed_sync_mod_list_for_app(app);
    enrich_failed_sync_mod_names(state, &mut list).await;
    list
}

#[tauri::command]
pub async fn list_failed_sync_mods_command(
    app: AppHandle,
    state: tauri::State<'_, crate::modio_client::ModioState>,
) -> Result<FailedSyncModList, String> {
    Ok(list_failed_sync_mods(&app, &state).await)
}

#[tauri::command]
pub async fn set_failed_sync_mod_ignored(
    app: AppHandle,
    state: tauri::State<'_, crate::modio_client::ModioState>,
    mod_id: u64,
    ignored: bool,
) -> Result<FailedSyncModList, String> {
    set_sync_mod_ignored(&app, mod_id, ignored)?;
    Ok(list_failed_sync_mods(&app, &state).await)
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
    Ok(list_failed_sync_mods(app, state).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_only_includes_recorded_failures() {
        let records = vec![
            FailedSyncModRecord {
                mod_id: 101,
                error_type: "install".to_string(),
                error_detail: Some("download failed".to_string()),
            },
            FailedSyncModRecord {
                mod_id: 202,
                error_type: "auth".to_string(),
                error_detail: None,
            },
        ];
        let list = build_failed_sync_mod_list(&records, &[202, 303]);
        assert_eq!(list.mods.len(), 2);
        assert_eq!(list.mods[0].mod_id, 101);
        assert_eq!(list.mods[0].error_type, "install");
        assert!(!list.mods[0].ignored);
        assert_eq!(list.mods[1].mod_id, 202);
        assert_eq!(list.mods[1].error_type, "auth");
        assert!(list.mods[1].ignored);
    }

    #[test]
    fn list_ignores_orphaned_ignore_entries() {
        let list = build_failed_sync_mod_list(&[], &[404]);
        assert!(list.mods.is_empty());
    }

    #[test]
    fn count_dependency_sync_failures_only_counts_dependency_type() {
        let records = vec![
            FailedSyncModRecord {
                mod_id: 101,
                error_type: "dependency".to_string(),
                error_detail: None,
            },
            FailedSyncModRecord {
                mod_id: 202,
                error_type: "install".to_string(),
                error_detail: None,
            },
            FailedSyncModRecord {
                mod_id: 303,
                error_type: "dependency".to_string(),
                error_detail: None,
            },
        ];
        let count = records
            .iter()
            .filter(|record| record.error_type == "dependency")
            .count() as u32;
        assert_eq!(count, 2);
    }

    #[test]
    fn refine_sync_failure_detects_rate_limit() {
        let (error_type, _) = refine_sync_failure("install", "OAuth rate limit reached");
        assert_eq!(error_type, "rate_limit");
    }

    #[test]
    fn refine_sync_failure_detects_auth_errors() {
        let (error_type, _) =
            refine_sync_failure("install_state", "The mod ID could not be found.");
        assert_eq!(error_type, "auth");
    }

    #[test]
    fn refine_sync_failure_maps_install_order_to_dependency() {
        let (error_type, detail) = refine_sync_failure("install_order", "Dependency graph error");
        assert_eq!(error_type, "dependency");
        assert_eq!(detail.as_deref(), Some("Dependency graph error"));
    }

    #[test]
    fn refine_sync_failure_maps_dependency_auth_to_dependency_detail() {
        let (error_type, detail) = refine_sync_failure(
            "install_order",
            "The mod ID could not be found. Sign in to mod.io to access private mods.",
        );
        assert_eq!(error_type, "dependency");
        assert_eq!(
            detail.as_deref(),
            Some("A required dependency is private or could not be loaded on mod.io.")
        );
    }

    #[test]
    fn merge_dependency_failure_details_accumulates_mod_ids() {
        let (error_type, detail) = merge_dependency_failure_details(
            Some("Required dependencies could not be installed (mods 3108325)."),
            "dependency",
            "Required dependency (mod 2518400): download failed",
        );
        assert_eq!(error_type, "dependency");
        assert_eq!(
            detail.as_deref(),
            Some("Required dependencies could not be installed (mods 2518400, 3108325).")
        );
    }
}
