use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

use crate::mod_api_cache::{ApiCache, PersistedCache};
use crate::modio_api::{ApiClient, ApiError, ModObject, ModQuery};

pub const AUTH_STORE_PATH: &str = "modio-auth.json";
const ACCESS_TOKEN_KEY: &str = "accessToken";
const USERNAME_KEY: &str = "username";

const CACHE_STORE_PATH: &str = "modio-cache.json";
const CACHE_PERSIST_KEY: &str = "cache";
const CACHE_SAVED_AT_KEY: &str = "savedAtUnix";
/// Persisted dependency cache is reused for a day before being re-fetched.
const CACHE_PERSIST_TTL_SECS: u64 = 24 * 60 * 60;

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

struct SessionData {
    token: Option<String>,
    username: Option<String>,
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
    api_client: Mutex<Option<Arc<ApiClient>>>,
    session: Mutex<SessionData>,
    api_cache: Mutex<ApiCache>,
    subscription_sync_cancelled: std::sync::atomic::AtomicBool,
    /// Serializes OAuth reads/writes so subscription sync and subscribe/unsubscribe
    /// never hit mod.io concurrently (avoids spurious global rate limits).
    oauth_request: tokio::sync::Mutex<()>,
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
            api_client: Mutex::new(None),
            session: Mutex::new(SessionData {
                token: None,
                username: None,
            }),
            api_cache: Mutex::new(ApiCache::default()),
            subscription_sync_cancelled: std::sync::atomic::AtomicBool::new(false),
            oauth_request: tokio::sync::Mutex::new(()),
        }
    }

    pub(crate) async fn with_oauth_request<T, F, Fut>(&self, label: &str, operation: F) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let _guard = self.oauth_request.lock().await;
        log::debug!("OAuth request: {label}");
        operation().await
    }

    pub(crate) fn cached_subscribed_mod_ids(&self) -> Option<Vec<u64>> {
        self.api_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get_subscribed_mod_ids())
    }

    pub(crate) fn store_subscribed_mod_ids(&self, mod_ids: Vec<u64>) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.store_subscribed_mod_ids(mod_ids);
        }
    }

    pub(crate) fn add_subscribed_mod_id(&self, mod_id: u64) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.add_subscribed_mod_id(mod_id);
        }
    }

    pub(crate) fn remove_subscribed_mod_id(&self, mod_id: u64) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.remove_subscribed_mod_id(mod_id);
        }
    }

    pub(crate) fn reset_subscription_sync_cancel(&self) {
        self.subscription_sync_cancelled
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn cancel_subscription_sync(&self) {
        log::debug!("Subscription sync cancellation requested");
        self.subscription_sync_cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn is_subscription_sync_cancelled(&self) -> bool {
        self.subscription_sync_cancelled
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub(crate) fn clear_api_cache(&self) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.clear();
        }
    }

    pub(crate) fn invalidate_mod_cache(&self, mod_id: u64) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.invalidate_mod(mod_id);
        }
    }

    pub(crate) fn cached_mod_unavailable(&self, mod_id: u64) -> bool {
        self.api_cache
            .lock()
            .ok()
            .is_some_and(|cache| cache.is_mod_unavailable(mod_id))
    }

    pub(crate) fn mark_mod_unavailable(&self, mod_id: u64) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.mark_mod_unavailable(mod_id);
        }
    }

    pub(crate) fn cached_dependencies(&self, mod_id: u64) -> Option<Vec<u64>> {
        let cache = self.api_cache.lock().ok()?;
        cache.get_dependencies(mod_id)
    }

    pub(crate) fn store_dependencies(&self, mod_id: u64, dependencies: Vec<u64>) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.store_dependencies(mod_id, dependencies);
        }
    }

    pub(crate) fn cached_latest_file_id(&self, mod_id: u64) -> Option<u64> {
        self.api_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get_latest_file_id(mod_id))
    }

    pub(crate) fn store_latest_file_id(&self, mod_id: u64, file_id: u64) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.store_latest_file_id(mod_id, file_id);
        }
    }

    pub(crate) fn cached_mod(&self, mod_id: u64) -> Option<ModObject> {
        self.api_cache.lock().ok().and_then(|cache| cache.get_mod(mod_id))
    }

    pub(crate) fn store_mod(&self, mod_: ModObject) {
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.store_mod(mod_);
        }
    }

    /// Loads the persisted dependency cache from disk (if recent enough). Called
    /// once at startup so a cold launch does not re-fetch dependencies.
    pub fn load_persisted_cache(&self, app: &AppHandle) {
        let Ok(store) = app.store(CACHE_STORE_PATH) else {
            return;
        };
        let saved_at = store
            .get(CACHE_SAVED_AT_KEY)
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        if saved_at == 0 || now_unix_secs().saturating_sub(saved_at) > CACHE_PERSIST_TTL_SECS {
            return;
        }
        let Some(value) = store.get(CACHE_PERSIST_KEY) else {
            return;
        };
        let Ok(snapshot) = serde_json::from_value::<PersistedCache>(value) else {
            return;
        };
        if let Ok(mut cache) = self.api_cache.lock() {
            cache.restore_persisted(snapshot);
        }
        log::debug!("Restored persisted dependency cache");
    }

    /// Writes the dependency cache to disk so the next launch can skip re-fetching.
    pub fn persist_cache(&self, app: &AppHandle) {
        let snapshot = match self.api_cache.lock() {
            Ok(cache) => cache.dependency_snapshot(),
            Err(_) => return,
        };
        let Ok(store) = app.store(CACHE_STORE_PATH) else {
            return;
        };
        store.set(CACHE_PERSIST_KEY, serde_json::json!(snapshot));
        store.set(CACHE_SAVED_AT_KEY, serde_json::json!(now_unix_secs()));
        if let Err(error) = store.save() {
            log::warn!("Failed to persist dependency cache: {error}");
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

    /// Shared REST client (built lazily once mod.io is configured). Used for all
    /// reads and writes; OAuth-authenticated calls pass the session token.
    pub(crate) fn api(&self) -> Result<Arc<ApiClient>, String> {
        let status = self.status();
        if !status.configured {
            return Err(status.message.unwrap_or_else(|| "mod.io is not configured".into()));
        }

        let mut guard = self
            .api_client
            .lock()
            .map_err(|_| "API client lock poisoned".to_string())?;

        if let Some(client) = guard.as_ref() {
            return Ok(client.clone());
        }

        let api_key = self.api_key.as_deref().unwrap();
        let client = Arc::new(ApiClient::new(
            api_key.to_string(),
            self.config.game_id,
            self.config.api_host.as_deref(),
            self.config.use_test_env,
        )?);
        *guard = Some(client.clone());
        Ok(client)
    }

    pub(crate) fn session_token(&self) -> Option<String> {
        self.session.lock().ok().and_then(|session| session.token.clone())
    }

    pub(crate) fn require_token(&self) -> Result<String, String> {
        self.session_token().ok_or_else(|| "Not logged in".to_string())
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
        }

        Ok(())
    }

    pub fn set_session(
        &self,
        app: &AppHandle,
        token: String,
        username: String,
    ) -> Result<(), String> {
        {
            let mut session = self.session.lock().unwrap();
            session.token = Some(token.clone());
            session.username = Some(username.clone());
        }

        let store = app.store(AUTH_STORE_PATH).map_err(|e| e.to_string())?;
        store.set(ACCESS_TOKEN_KEY, serde_json::json!(token));
        store.set(USERNAME_KEY, serde_json::json!(username));
        store.save().map_err(|e| e.to_string())?;

        log::info!("mod.io session started for {username}");
        Ok(())
    }

    pub fn clear_session(&self, app: &AppHandle) -> Result<(), String> {
        let username = self
            .auth_status()
            .username
            .unwrap_or_else(|| "user".to_string());
        {
            let mut session = self.session.lock().unwrap();
            session.token = None;
            session.username = None;
        }
        self.clear_api_cache();

        let store = app.store(AUTH_STORE_PATH).map_err(|e| e.to_string())?;
        let _ = store.delete(ACCESS_TOKEN_KEY);
        let _ = store.delete(USERNAME_KEY);
        store.save().map_err(|e| e.to_string())?;

        log::info!("mod.io session cleared for {username}");
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
}

/// Converts an `ApiError` into a user-facing message, preserving the rate-limit
/// and auth hints we surfaced previously.
pub fn format_api_error(error: ApiError) -> String {
    if error.is_rate_limited() {
        let error_ref = error
            .error_ref
            .map(|code| format!(" (error_ref {code})"))
            .unwrap_or_default();
        return format!(
            "{}{error_ref} This applies to your mod.io login session (OAuth). Wait about a minute, then try again.",
            error.message
        );
    }
    if error.is_auth() {
        return format!(
            "{}. Email login requires your game's API key and MODIO_GAME_ID — not a personal read-only key from mod.io/me/access.",
            error.message
        );
    }
    error.message
}

/// True when mod.io reports the mod no longer exists (deleted or removed).
pub fn is_mod_unavailable(error: &ApiError) -> bool {
    error.is_not_found()
}

pub(crate) fn is_rate_limited_message(message: &str) -> bool {
    message.to_ascii_lowercase().contains("rate limit")
}

pub(crate) async fn with_rate_limit_retry<T, F, Fut>(mut operation: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    match operation().await {
        Ok(value) => Ok(value),
        Err(message) if is_rate_limited_message(&message) => {
            log::warn!("mod.io rate limit hit, retrying in 61 seconds: {message}");
            tokio::time::sleep(Duration::from_secs(61)).await;
            operation().await
        }
        Err(message) => Err(message),
    }
}

pub(crate) async fn subscribe_to_mod(state: &ModioState, mod_id: u64) -> Result<(), String> {
    log::info!("Subscribing to mod {mod_id}");
    state
        .with_oauth_request("subscribe_to_mod", || async {
            let game_id = state.game_id()?;
            let api = state.api()?;
            let token = state.require_token()?;
            match api.subscribe(&token, game_id, mod_id).await {
                Ok(()) => {
                    state.add_subscribed_mod_id(mod_id);
                    log::info!("Subscribed to mod {mod_id}");
                    Ok(())
                }
                Err(error) if error.is_conflict() => {
                    state.add_subscribed_mod_id(mod_id);
                    log::debug!("Mod {mod_id} already subscribed");
                    Ok(())
                }
                Err(error) => Err(format_api_error(error)),
            }
        })
        .await
}

pub(crate) async fn unsubscribe_from_mod(state: &ModioState, mod_id: u64) -> Result<(), String> {
    log::info!("Unsubscribing from mod {mod_id}");
    state
        .with_oauth_request("unsubscribe_from_mod", || async {
            let game_id = state.game_id()?;
            let api = state.api()?;
            let token = state.require_token()?;
            match api.unsubscribe(&token, game_id, mod_id).await {
                Ok(()) => {
                    state.remove_subscribed_mod_id(mod_id);
                    log::info!("Unsubscribed from mod {mod_id}");
                    Ok(())
                }
                Err(error) if error.is_not_found() || error.is_not_subscribed() => {
                    state.remove_subscribed_mod_id(mod_id);
                    log::debug!("Mod {mod_id} was not subscribed");
                    Ok(())
                }
                Err(error) => Err(format_api_error(error)),
            }
        })
        .await
}

pub(crate) async fn fetch_subscribed_mod_ids(state: &ModioState) -> Result<Vec<u64>, String> {
    if let Some(cached) = state.cached_subscribed_mod_ids() {
        log::debug!("Using cached subscription list ({} mod(s))", cached.len());
        return Ok(cached);
    }

    let mod_ids = state
        .with_oauth_request("fetch_subscribed_mod_ids", || async {
            const PAGE_LIMIT: u32 = 100;
            let game_id = state.game_id()?;
            let api = state.api()?;
            let token = state.require_token()?;

            let mut mod_ids: Vec<u64> = Vec::new();
            let mut offset: u32 = 0;
            loop {
                let list = api
                    .get_user_subscriptions(&token, game_id, PAGE_LIMIT, offset)
                    .await
                    .map_err(format_api_error)?;
                let count = list.data.len() as u32;
                let total = list.result_total;
                // The subscriptions response already contains full mod objects.
                // Seed the caches so the install/sync flow doesn't re-fetch each
                // mod, its latest file, or (for dependency-free mods) its deps.
                for mod_ in list.data {
                    let mod_id = mod_.id;
                    mod_ids.push(mod_id);
                    if let Some(file) = &mod_.modfile {
                        state.store_latest_file_id(mod_id, file.id);
                    }
                    if !mod_.dependencies {
                        state.store_dependencies(mod_id, Vec::new());
                    }
                    state.store_mod(mod_);
                }
                offset = offset.saturating_add(count);
                if count < PAGE_LIMIT || count == 0 || offset >= total {
                    break;
                }
            }

            mod_ids.sort_unstable();
            mod_ids.dedup();
            Ok(mod_ids)
        })
        .await?;

    state.store_subscribed_mod_ids(mod_ids.clone());
    log::info!("Found {} subscribed mod(s)", mod_ids.len());
    Ok(mod_ids)
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

fn mod_sort_field(sort: ModSort) -> &'static str {
    match sort {
        // https://docs.mod.io/restapiref/#get-mods
        ModSort::RecentlyAdded => "date_live",
        ModSort::LastUpdated => "date_updated",
        ModSort::Trending => "downloads_today",
        ModSort::MostPopular => "downloads_total",
        ModSort::MostSubscribers => "subscribers_total",
        ModSort::HighestRated => "ratings_weighted_aggregate",
        ModSort::Alphabetical => "name",
    }
}

fn build_mod_query(params: &ListModsParams) -> ModQuery {
    let mut query = ModQuery {
        limit: params.limit.clamp(1, 100),
        offset: params.offset,
        sort_field: Some(mod_sort_field(params.sort)),
        sort_desc: matches!(params.sort_dir, SortDir::Desc),
        ..Default::default()
    };

    if let Some(search) = params
        .search
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        query.search = Some(search.to_string());
    }

    if let Some(tag) = mod_type_tag(params.mod_type) {
        query.tags.push(tag.to_string());
    }

    if !params.category_tags.is_empty() {
        if params.category_tags.len() == 1 {
            query.tags.push(params.category_tags[0].clone());
        } else {
            query.tags_in = params.category_tags.clone();
        }
    }

    query
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

fn mod_to_summary(mod_: ModObject) -> ModSummary {
    ModSummary {
        id: mod_.id,
        name: mod_.name,
        summary: mod_.summary,
        profile_url: mod_.profile_url,
        logo_url: mod_.logo.thumb_320x180,
        downloads_total: mod_.stats.downloads_total,
        subscribers_total: mod_.stats.subscribers_total,
        popularity_rank: Some(mod_.stats.popularity_rank_position),
        tags: mod_.tags.into_iter().map(|tag| tag.name).collect(),
        date_updated: timestamp_to_iso(mod_.date_updated),
    }
}

fn mod_to_detail(mod_: ModObject) -> ModDetail {
    let media_image_urls: Vec<String> = mod_
        .media
        .images
        .iter()
        .map(|image| image.original.clone())
        .collect();
    let hero_image_url = media_image_urls
        .first()
        .cloned()
        .unwrap_or_else(|| mod_.logo.original.clone());
    let file_id = mod_.modfile.as_ref().map(|file| file.id);
    let submitted_by_avatar_url = mod_.submitted_by.avatar_thumb();
    let tags: Vec<String> = mod_.tags.into_iter().map(|tag| tag.name).collect();

    ModDetail {
        id: mod_.id,
        name: mod_.name,
        summary: mod_.summary,
        profile_url: mod_.profile_url,
        logo_url: mod_.logo.thumb_320x180,
        hero_image_url,
        downloads_total: mod_.stats.downloads_total,
        downloads_today: mod_.stats.downloads_today,
        subscribers_total: mod_.stats.subscribers_total,
        popularity_rank: Some(mod_.stats.popularity_rank_position),
        tags,
        date_added: timestamp_to_iso(mod_.date_added),
        date_updated: timestamp_to_iso(mod_.date_updated),
        date_live: timestamp_to_iso(mod_.date_live),
        description_html: mod_.description,
        submitted_by_username: mod_.submitted_by.username,
        submitted_by_profile_url: mod_.submitted_by.profile_url,
        submitted_by_avatar_url,
        ratings_display_text: mod_.stats.ratings_display_text,
        ratings_percentage_positive: mod_.stats.ratings_percentage_positive,
        ratings_positive: mod_.stats.ratings_positive,
        ratings_negative: mod_.stats.ratings_negative,
        media_image_urls,
        has_dependencies: mod_.dependencies,
        homepage_url: mod_.homepage_url,
        file_id,
    }
}

fn mod_to_dependency(mod_: ModObject) -> ModDependency {
    let file_size_bytes = mod_.modfile.as_ref().map(|file| file.filesize);
    ModDependency {
        id: mod_.id,
        name: mod_.name,
        profile_url: mod_.profile_url,
        logo_url: mod_.logo.thumb_320x180,
        submitted_by_username: mod_.submitted_by.username,
        date_updated: timestamp_to_iso(mod_.date_updated),
        downloads_total: mod_.stats.downloads_total,
        file_size_bytes,
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
    let api = state.api()?;
    let list = api.get_game_tags(game_id).await.map_err(format_api_error)?;

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
    let api = state.api()?;
    let query = build_mod_query(&params);
    let list = api.get_mods(game_id, &query).await.map_err(format_api_error)?;

    let total = list.result_total;
    let mods = list.data.into_iter().map(mod_to_summary).collect();

    Ok(ModListResult { mods, total })
}

#[tauri::command]
pub async fn get_mod(state: State<'_, ModioState>, mod_id: u64) -> Result<ModDetail, String> {
    let game_id = state.game_id()?;
    let api = state.api()?;
    let mod_ = api.get_mod(game_id, mod_id).await.map_err(format_api_error)?;

    Ok(mod_to_detail(mod_))
}

#[tauri::command]
pub async fn list_mod_dependencies(
    state: State<'_, ModioState>,
    mod_id: u64,
) -> Result<ModDependencyListResult, String> {
    let game_id = state.game_id()?;
    let api = state.api()?;
    let list = api
        .get_mod_dependencies(game_id, mod_id)
        .await
        .map_err(format_api_error)?;

    let mut mods = Vec::with_capacity(list.data.len());
    for dependency in list.data {
        let mod_ = api
            .get_mod(game_id, dependency.mod_id)
            .await
            .map_err(format_api_error)?;
        mods.push(mod_to_dependency(mod_));
    }

    Ok(ModDependencyListResult {
        total: mods.len() as u32,
        mods,
    })
}

#[tauri::command]
pub async fn get_user_profile(state: State<'_, ModioState>) -> Result<UserProfile, String> {
    let api = state.api()?;
    let token = state.require_token()?;
    let user = api
        .get_authenticated_user(&token)
        .await
        .map_err(format_api_error)?;

    Ok(UserProfile {
        avatar_url: user.avatar_thumb(),
        username: user.username,
        profile_url: user.profile_url,
    })
}

#[tauri::command]
pub async fn list_user_mods(state: State<'_, ModioState>) -> Result<ModListResult, String> {
    let game_id = state.game_id()?;
    let api = state.api()?;
    let token = state.require_token()?;
    let list = api
        .get_user_mods(&token, game_id)
        .await
        .map_err(format_api_error)?;

    let total = list.result_total;
    let mods = list.data.into_iter().map(mod_to_summary).collect();

    Ok(ModListResult { mods, total })
}
