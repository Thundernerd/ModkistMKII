use std::sync::{Arc, Mutex};

use modio::request::filter::prelude::*;
use modio::request::filter::{custom_order_by_asc, custom_order_by_desc, Filter};
use modio::types::id::Id;
use modio::types::TargetPlatform;
use modio::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

pub const AUTH_STORE_PATH: &str = "modio-auth.json";
const ACCESS_TOKEN_KEY: &str = "accessToken";
const USERNAME_KEY: &str = "username";

struct SessionData {
    token: Option<String>,
    username: Option<String>,
    client: Option<Arc<Client>>,
}

#[derive(Clone)]
struct ModioConfig {
    game_id: Option<u64>,
    api_host: Option<String>,
    use_test_env: bool,
}

impl ModioConfig {
    fn from_env() -> Self {
        let game_id = std::env::var("MODIO_GAME_ID")
            .ok()
            .and_then(|value| value.trim().parse().ok());
        let api_host = std::env::var("MODIO_API_HOST")
            .ok()
            .map(|value| value.trim().trim_start_matches("https://").to_string())
            .filter(|value| !value.is_empty());
        let use_test_env = std::env::var("MODIO_USE_TEST_ENV")
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes"));

        Self {
            game_id,
            api_host,
            use_test_env,
        }
    }

    fn has_host(&self) -> bool {
        self.game_id.is_some() || self.api_host.is_some() || self.use_test_env
    }
}

pub struct ModioState {
    api_key: Option<String>,
    config: ModioConfig,
    base_client: Mutex<Option<Arc<Client>>>,
    session: Mutex<SessionData>,
}

impl ModioState {
    pub fn from_env() -> Self {
        let api_key = std::env::var("MODIO_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Self {
            api_key,
            config: ModioConfig::from_env(),
            base_client: Mutex::new(None),
            session: Mutex::new(SessionData {
                token: None,
                username: None,
                client: None,
            }),
        }
    }

    pub fn status(&self) -> ModioStatus {
        if self.api_key.is_none() {
            return ModioStatus {
                configured: false,
                message: Some(
                    "Set MODIO_API_KEY in .env (use your game's API key from mod.io).".into(),
                ),
            };
        }

        if !self.config.has_host() {
            return ModioStatus {
                configured: false,
                message: Some(
                    "Set MODIO_GAME_ID in .env (found on your game's mod.io dashboard).".into(),
                ),
            };
        }

        ModioStatus {
            configured: true,
            message: None,
        }
    }

    pub(crate) fn get_base_client(&self) -> Result<Arc<Client>, String> {
        let status = self.status();
        if !status.configured {
            return Err(status.message.unwrap_or_else(|| "mod.io is not configured".into()));
        }

        let mut guard = self
            .base_client
            .lock()
            .map_err(|_| "base client lock poisoned".to_string())?;

        if let Some(client) = guard.as_ref() {
            return Ok(client.clone());
        }

        let api_key = self.api_key.as_deref().unwrap();
        let client = Arc::new(build_client(api_key, &self.config)?);
        *guard = Some(client.clone());
        Ok(client)
    }

    pub fn get_mods_client(&self) -> Result<Arc<Client>, String> {
        if self.auth_status().logged_in {
            self.get_session_client()
        } else {
            self.get_base_client()
        }
    }

    pub fn get_session_client(&self) -> Result<Arc<Client>, String> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;

        if let Some(client) = session.client.as_ref() {
            return Ok(client.clone());
        }

        let token = session
            .token
            .clone()
            .ok_or_else(|| "Not logged in".to_string())?;

        let base = self.get_base_client()?;
        let client = Arc::new(base.with_token(token));
        session.client = Some(client.clone());
        Ok(client)
    }

    pub fn auth_status(&self) -> AuthStatus {
        let session = self.session.lock().unwrap();
        AuthStatus {
            logged_in: session.token.is_some(),
            username: session.username.clone(),
        }
    }

    pub fn restore_from_store(&self, app: &AppHandle) -> Result<(), String> {
        let store = app.store(AUTH_STORE_PATH).map_err(|e| e.to_string())?;
        let token = store
            .get(ACCESS_TOKEN_KEY)
            .and_then(|value| value.as_str().map(str::to_string));
        let username = store
            .get(USERNAME_KEY)
            .and_then(|value| value.as_str().map(str::to_string));

        if token.is_some() {
            let mut session = self.session.lock().unwrap();
            session.token = token;
            session.username = username;
            session.client = None;
        }

        Ok(())
    }

    pub fn set_session(
        &self,
        app: &AppHandle,
        token: String,
        username: String,
    ) -> Result<(), String> {
        let base = self.get_base_client()?;
        {
            let mut session = self.session.lock().unwrap();
            session.token = Some(token.clone());
            session.username = Some(username.clone());
            session.client = Some(Arc::new(base.with_token(token.clone())));
        }

        let store = app.store(AUTH_STORE_PATH).map_err(|e| e.to_string())?;
        store.set(ACCESS_TOKEN_KEY, serde_json::json!(token));
        store.set(USERNAME_KEY, serde_json::json!(username));
        store.save().map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn clear_session(&self, app: &AppHandle) -> Result<(), String> {
        {
            let mut session = self.session.lock().unwrap();
            session.token = None;
            session.username = None;
            session.client = None;
        }

        let store = app.store(AUTH_STORE_PATH).map_err(|e| e.to_string())?;
        let _ = store.delete(ACCESS_TOKEN_KEY);
        let _ = store.delete(USERNAME_KEY);
        store.save().map_err(|e| e.to_string())?;

        Ok(())
    }

    fn game_id(&self) -> Result<u64, String> {
        self.config
            .game_id
            .ok_or_else(|| "MODIO_GAME_ID is not set in .env".to_string())
    }
}

fn build_client(api_key: &str, config: &ModioConfig) -> Result<Client, String> {
    let mut builder = Client::builder(api_key.to_string());

    if config.use_test_env {
        builder = builder.use_test_env();
    } else if let Some(host) = &config.api_host {
        builder = builder.host(host.as_str());
    } else if let Some(game_id) = config.game_id {
        builder = builder.game_host(Id::new(game_id));
    } else {
        return Err(
            "MODIO_GAME_ID or MODIO_API_HOST is required for mod.io authentication".into(),
        );
    }

    builder = builder.target_platform(current_target_platform());

    builder.build().map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn current_target_platform() -> TargetPlatform {
    TargetPlatform::MAC
}

#[cfg(target_os = "linux")]
fn current_target_platform() -> TargetPlatform {
    TargetPlatform::LINUX
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn current_target_platform() -> TargetPlatform {
    TargetPlatform::WINDOWS
}

pub fn format_modio_error(error: modio::Error) -> String {
    let message = error.to_string();
    if error.is_auth() {
        return format!(
            "{message}. Email login requires your game's API key and MODIO_GAME_ID — not a personal read-only key from mod.io/me/access."
        );
    }
    message
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModioStatus {
    pub configured: bool,
    pub message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub logged_in: bool,
    pub username: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSummary {
    pub id: u64,
    pub name: String,
    pub summary: String,
    pub profile_url: String,
    pub logo_url: String,
    pub downloads_total: u32,
    pub subscribers_total: u32,
    pub popularity_rank: Option<u32>,
    pub tags: Vec<String>,
    pub date_updated: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModListResult {
    pub mods: Vec<ModSummary>,
    pub total: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListModsParams {
    pub search: Option<String>,
    #[serde(default)]
    pub sort: ModSort,
    #[serde(default)]
    pub sort_dir: SortDir,
    #[serde(default = "default_mod_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_mod_limit() -> u32 {
    20
}

#[derive(Deserialize, Default, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ModSort {
    RecentlyAdded,
    LastUpdated,
    #[default]
    Trending,
    MostPopular,
    MostSubscribers,
    HighestRated,
    Alphabetical,
}

#[derive(Deserialize, Default, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SortDir {
    #[default]
    Desc,
    Asc,
}

/// Higher values first when `descending` is true (downloads, ratings, dates, etc.).
fn sort_by_value(field: &'static str, descending: bool) -> Filter {
    if descending {
        custom_order_by_desc(field)
    } else {
        custom_order_by_asc(field)
    }
}

fn mod_sort_filter(sort: ModSort, dir: SortDir) -> Filter {
    let descending = matches!(dir, SortDir::Desc);

    match sort {
        // https://docs.mod.io/restapiref/#get-mods
        ModSort::RecentlyAdded => sort_by_value("date_live", descending),
        ModSort::LastUpdated => sort_by_value("date_updated", descending),
        ModSort::Trending => sort_by_value("downloads_today", descending),
        ModSort::MostPopular => sort_by_value("downloads_total", descending),
        ModSort::MostSubscribers => sort_by_value("subscribers_total", descending),
        ModSort::HighestRated => sort_by_value("ratings_weighted_aggregate", descending),
        ModSort::Alphabetical => sort_by_value("name", descending),
    }
}

fn build_mod_filter(params: &ListModsParams) -> Filter {
    let limit = params.limit.clamp(1, 100) as usize;
    let offset = params.offset as usize;

    let mut filter = with_limit(limit).offset(offset);

    if let Some(search) = params
        .search
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        filter = filter.and(Fulltext::eq(search.to_string()));
    }

    filter.order_by(mod_sort_filter(params.sort, params.sort_dir))
}

fn timestamp_to_iso(secs: i64) -> String {
    if secs <= 0 {
        return String::new();
    }
    time::OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|dt| {
            dt.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_default()
}

#[tauri::command]
pub fn modio_status(state: State<'_, ModioState>) -> ModioStatus {
    state.status()
}

#[tauri::command]
pub async fn list_mods(
    state: State<'_, ModioState>,
    params: ListModsParams,
) -> Result<ModListResult, String> {
    let game_id = state.game_id()?;
    let client = state.get_mods_client()?;
    let filter = build_mod_filter(&params);
    let response = client
        .get_mods(Id::new(game_id))
        .filter(filter)
        .await
        .map_err(format_modio_error)?;
    let list = response.data().await.map_err(|e| e.to_string())?;

    let mods = list
        .data
        .into_iter()
        .map(|mod_| ModSummary {
            id: mod_.id.get(),
            name: mod_.name,
            summary: mod_.summary,
            profile_url: mod_.profile_url.to_string(),
            logo_url: mod_.logo.thumb_320x180.to_string(),
            downloads_total: mod_.stats.downloads_total,
            subscribers_total: mod_.stats.subscribers_total,
            popularity_rank: Some(mod_.stats.popularity.rank_position),
            tags: mod_.tags.into_iter().map(|tag| tag.name).collect(),
            date_updated: timestamp_to_iso(mod_.date_updated.as_secs()),
        })
        .collect();

    Ok(ModListResult {
        mods,
        total: list.total,
    })
}
