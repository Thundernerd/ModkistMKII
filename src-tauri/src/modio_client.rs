use std::sync::{Arc, Mutex};

use modio::request::filter::prelude::*;
use modio::request::filter::{custom_filter, custom_order_by_asc, custom_order_by_desc, Filter, Operator};
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

    pub(crate) fn game_id(&self) -> Result<u64, String> {
        self.config
            .game_id
            .ok_or_else(|| "MODIO_GAME_ID is not set in .env".to_string())
    }

    pub(crate) fn api_key(&self) -> Result<&str, String> {
        self.api_key
            .as_deref()
            .ok_or_else(|| "MODIO_API_KEY is not set in .env".to_string())
    }

    pub(crate) fn access_token(&self) -> Option<String> {
        let session = self.session.lock().ok()?;
        session.token.clone()
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

    builder = builder.target_platform(game_target_platform());

    builder.build().map_err(|e| e.to_string())
}

fn game_target_platform() -> TargetPlatform {
    // Zeepkist is a Windows game; modfiles must be resolved for Windows even when
    // Modkist itself runs on macOS or Linux during development.
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
pub struct ModDetail {
    pub id: u64,
    pub name: String,
    pub summary: String,
    pub profile_url: String,
    pub logo_url: String,
    pub hero_image_url: String,
    pub downloads_total: u32,
    pub downloads_today: u32,
    pub subscribers_total: u32,
    pub popularity_rank: Option<u32>,
    pub tags: Vec<String>,
    pub date_added: String,
    pub date_updated: String,
    pub date_live: String,
    pub description_html: Option<String>,
    pub submitted_by_username: String,
    pub submitted_by_profile_url: String,
    pub submitted_by_avatar_url: Option<String>,
    pub ratings_display_text: String,
    pub ratings_percentage_positive: u32,
    pub ratings_positive: u32,
    pub ratings_negative: u32,
    pub media_image_urls: Vec<String>,
    pub has_dependencies: bool,
    pub homepage_url: Option<String>,
    pub file_id: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependency {
    pub id: u64,
    pub name: String,
    pub profile_url: String,
    pub logo_url: String,
    pub submitted_by_username: String,
    pub date_updated: String,
    pub downloads_total: u32,
    pub file_size_bytes: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyListResult {
    pub mods: Vec<ModDependency>,
    pub total: u32,
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
    pub mod_type: ModTypeFilter,
    #[serde(default)]
    pub category_tags: Vec<String>,
    #[serde(default)]
    pub sort: ModSort,
    #[serde(default)]
    pub sort_dir: SortDir,
    #[serde(default = "default_mod_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Deserialize, Default, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ModTypeFilter {
    #[default]
    All,
    Plugin,
    Blueprint,
}

fn mod_type_tag(filter: ModTypeFilter) -> Option<&'static str> {
    match filter {
        ModTypeFilter::All => None,
        ModTypeFilter::Plugin => Some("Plugin"),
        ModTypeFilter::Blueprint => Some("Blueprint"),
    }
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

    if let Some(tag) = mod_type_tag(params.mod_type) {
        filter = filter.and(custom_filter("tags", Operator::Equals, tag));
    }

    if !params.category_tags.is_empty() {
        if params.category_tags.len() == 1 {
            filter = filter.and(custom_filter(
                "tags",
                Operator::Equals,
                params.category_tags[0].clone(),
            ));
        } else {
            filter = filter.and(custom_filter(
                "tags",
                Operator::In,
                params.category_tags.clone(),
            ));
        }
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

fn mod_to_summary(mod_: modio::types::mods::Mod) -> ModSummary {
    ModSummary {
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
    }
}

fn mod_to_detail(mod_: modio::types::mods::Mod) -> ModDetail {
    let media_image_urls: Vec<String> = mod_
        .media
        .images
        .iter()
        .map(|image| image.original.to_string())
        .collect();
    let hero_image_url = media_image_urls
        .first()
        .cloned()
        .unwrap_or_else(|| mod_.logo.original.to_string());

    ModDetail {
        id: mod_.id.get(),
        name: mod_.name,
        summary: mod_.summary,
        profile_url: mod_.profile_url.to_string(),
        logo_url: mod_.logo.thumb_320x180.to_string(),
        hero_image_url,
        downloads_total: mod_.stats.downloads_total,
        downloads_today: mod_.stats.downloads_today,
        subscribers_total: mod_.stats.subscribers_total,
        popularity_rank: Some(mod_.stats.popularity.rank_position),
        tags: mod_.tags.into_iter().map(|tag| tag.name).collect(),
        date_added: timestamp_to_iso(mod_.date_added.as_secs()),
        date_updated: timestamp_to_iso(mod_.date_updated.as_secs()),
        date_live: timestamp_to_iso(mod_.date_live.as_secs()),
        description_html: mod_.description,
        submitted_by_username: mod_.submitted_by.username,
        submitted_by_profile_url: mod_.submitted_by.profile_url.to_string(),
        submitted_by_avatar_url: mod_
            .submitted_by
            .avatar
            .as_ref()
            .map(|avatar| avatar.thumb_100x100.to_string()),
        ratings_display_text: mod_.stats.ratings.display_text,
        ratings_percentage_positive: mod_.stats.ratings.percentage_positive,
        ratings_positive: mod_.stats.ratings.positive,
        ratings_negative: mod_.stats.ratings.negative,
        media_image_urls,
        has_dependencies: mod_.dependencies,
        homepage_url: mod_.homepage_url.map(|url| url.to_string()),
        file_id: mod_.modfile.as_ref().map(|file| file.id.get()),
    }
}

fn mod_to_dependency(mod_: modio::types::mods::Mod) -> ModDependency {
    ModDependency {
        id: mod_.id.get(),
        name: mod_.name,
        profile_url: mod_.profile_url.to_string(),
        logo_url: mod_.logo.thumb_320x180.to_string(),
        submitted_by_username: mod_.submitted_by.username,
        date_updated: timestamp_to_iso(mod_.date_updated.as_secs()),
        downloads_total: mod_.stats.downloads_total,
        file_size_bytes: mod_.modfile.as_ref().map(|file| file.filesize),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub username: String,
    pub profile_url: String,
    pub avatar_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModTagOptions {
    pub plugin_types: Vec<String>,
    pub blueprint_types: Vec<String>,
}

#[tauri::command]
pub fn modio_status(state: State<'_, ModioState>) -> ModioStatus {
    state.status()
}

#[tauri::command]
pub async fn get_mod_tag_options(state: State<'_, ModioState>) -> Result<ModTagOptions, String> {
    let game_id = state.game_id()?;
    let client = state.get_base_client()?;
    let response = client
        .get_game_tags(Id::new(game_id))
        .await
        .map_err(format_modio_error)?;
    let list = response.data().await.map_err(|e| e.to_string())?;

    let mut plugin_types = Vec::new();
    let mut blueprint_types = Vec::new();

    for option in list.data {
        match option.name.as_str() {
            "Plugin Type" => plugin_types = option.tags,
            "Blueprint Type" => blueprint_types = option.tags,
            _ => {}
        }
    }

    Ok(ModTagOptions {
        plugin_types,
        blueprint_types,
    })
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

    let mods = list.data.into_iter().map(mod_to_summary).collect();

    Ok(ModListResult {
        mods,
        total: list.total,
    })
}

#[tauri::command]
pub async fn get_mod(state: State<'_, ModioState>, mod_id: u64) -> Result<ModDetail, String> {
    let game_id = state.game_id()?;
    let client = state.get_mods_client()?;
    let response = client
        .get_mod(Id::new(game_id), Id::new(mod_id))
        .await
        .map_err(format_modio_error)?;
    let mod_ = response.data().await.map_err(|e| e.to_string())?;

    Ok(mod_to_detail(mod_))
}

#[tauri::command]
pub async fn list_mod_dependencies(
    state: State<'_, ModioState>,
    mod_id: u64,
) -> Result<ModDependencyListResult, String> {
    let game_id = state.game_id()?;
    let client = state.get_mods_client()?;
    let response = client
        .get_mod_dependencies(Id::new(game_id), Id::new(mod_id))
        .await
        .map_err(format_modio_error)?;
    let list = response.data().await.map_err(|e| e.to_string())?;

    let mut mods = Vec::with_capacity(list.data.len());
    for dependency in list.data {
        let mod_response = client
            .get_mod(Id::new(game_id), dependency.mod_id)
            .await
            .map_err(format_modio_error)?;
        let mod_ = mod_response.data().await.map_err(|e| e.to_string())?;
        mods.push(mod_to_dependency(mod_));
    }

    Ok(ModDependencyListResult {
        total: mods.len() as u32,
        mods,
    })
}

#[tauri::command]
pub async fn get_user_profile(state: State<'_, ModioState>) -> Result<UserProfile, String> {
    let client = state.get_session_client()?;
    let response = client
        .get_authenticated_user()
        .await
        .map_err(format_modio_error)?;
    let user = response.data().await.map_err(|e| e.to_string())?;

    Ok(UserProfile {
        username: user.username,
        profile_url: user.profile_url.to_string(),
        avatar_url: user
            .avatar
            .as_ref()
            .map(|avatar| avatar.thumb_100x100.to_string()),
    })
}

#[tauri::command]
pub async fn list_user_mods(state: State<'_, ModioState>) -> Result<ModListResult, String> {
    let game_id = state.game_id()?;
    let client = state.get_session_client()?;
    let filter = custom_filter("game_id", Operator::Equals, game_id.to_string());
    let response = client
        .get_user_mods()
        .filter(filter)
        .await
        .map_err(format_modio_error)?;
    let list = response.data().await.map_err(|e| e.to_string())?;

    let mods = list.data.into_iter().map(mod_to_summary).collect();

    Ok(ModListResult {
        mods,
        total: list.total,
    })
}
