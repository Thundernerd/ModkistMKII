//! Minimal mod.io v1 REST client built on `reqwest` + `serde`.
//!
//! Replaces the `modio` crate so we control headers, error handling, logging
//! and rate-limit behavior directly. All requests target a single host
//! (`https://g-{game_id}.modapi.io/v1` by default). Mod metadata and dependency
//! reads try the game `api_key` first; when that fails and a bearer token is
//! available, the same request is retried with OAuth (e.g. private subscribed
//! mods). Other OAuth-only endpoints always use the bearer token.

use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Deserialize;

const PLATFORM_HEADER: &str = "windows";
const DEFAULT_HOST: &str = "api.mod.io";
const TEST_HOST: &str = "api.test.mod.io";
const USER_AGENT: &str = concat!("Modkist/", env!("CARGO_PKG_VERSION"));

/// Minimum spacing between any two mod.io requests. mod.io enforces a global
/// "too many requests in a short period" limiter (error_ref 11008) that trips on
/// bursts even when the per-minute quota is fine, so we pace every call.
const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(110);

/// Error returned by any mod.io API call. Captures the HTTP status, the mod.io
/// `error_ref`, the server message and the `Retry-After` value (seconds) so the
/// caller can distinguish rate limits, auth failures and missing resources.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: Option<u16>,
    pub error_ref: Option<u32>,
    pub message: String,
    pub retry_after_secs: Option<u64>,
}

impl ApiError {
    fn transport(message: String) -> Self {
        Self {
            status: None,
            error_ref: None,
            message,
            retry_after_secs: None,
        }
    }

    /// True when mod.io is rate limiting us (HTTP 429, or the global/per-endpoint
    /// `error_ref` codes 11008/11009).
    pub fn is_rate_limited(&self) -> bool {
        self.status == Some(429) || matches!(self.error_ref, Some(11008 | 11009))
    }

    pub fn is_auth(&self) -> bool {
        matches!(self.status, Some(401 | 403))
    }

    pub fn is_not_found(&self) -> bool {
        self.status == Some(404)
    }

    pub fn is_conflict(&self) -> bool {
        self.status == Some(409)
    }

    /// True when mod.io reports the user is not subscribed to the mod
    /// (error_ref 15005, returned as HTTP 400 by the unsubscribe endpoint).
    /// Unsubscribing is then a no-op, so callers can treat it as success.
    pub fn is_not_subscribed(&self) -> bool {
        self.error_ref == Some(15005)
    }

    fn log(&self, context: &str) {
        log::error!(
            "mod.io API error [{context}]: {} (status={:?}, error_ref={:?}, retry_after_secs={:?})",
            self.message,
            self.status,
            self.error_ref,
            self.retry_after_secs
        );
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error_ref: Option<u32>,
    #[serde(default)]
    message: String,
}

/// Standard paginated mod.io list response.
#[derive(Deserialize)]
pub struct ListResponse<T> {
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
    #[serde(default)]
    pub result_total: u32,
}

#[derive(Deserialize, Default, Clone)]
pub struct Logo {
    #[serde(default)]
    pub original: String,
    #[serde(default)]
    pub thumb_320x180: String,
}

#[derive(Deserialize, Default, Clone)]
pub struct Avatar {
    #[serde(default)]
    pub thumb_100x100: String,
}

#[derive(Deserialize, Default, Clone)]
pub struct UserObject {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub profile_url: String,
    #[serde(default)]
    pub avatar: Option<Avatar>,
}

impl UserObject {
    /// Avatar thumbnail URL, or `None` when the user has no avatar (mod.io may
    /// return an empty avatar object).
    pub fn avatar_thumb(&self) -> Option<String> {
        self.avatar
            .as_ref()
            .map(|avatar| avatar.thumb_100x100.clone())
            .filter(|url| !url.is_empty())
    }
}

#[derive(Deserialize, Default, Clone)]
pub struct Media {
    #[serde(default)]
    pub images: Vec<MediaImage>,
}

#[derive(Deserialize, Clone)]
pub struct MediaImage {
    #[serde(default)]
    pub original: String,
}

#[derive(Deserialize, Clone)]
pub struct TagObject {
    #[serde(default)]
    pub name: String,
}

#[derive(Deserialize, Default, Clone)]
pub struct ModStats {
    #[serde(default)]
    pub downloads_today: u32,
    #[serde(default)]
    pub downloads_total: u32,
    #[serde(default)]
    pub subscribers_total: u32,
    #[serde(default)]
    pub popularity_rank_position: u32,
    #[serde(default)]
    pub ratings_positive: u32,
    #[serde(default)]
    pub ratings_negative: u32,
    #[serde(default)]
    pub ratings_percentage_positive: u32,
    #[serde(default)]
    pub ratings_display_text: String,
}

#[derive(Deserialize, Default, Clone)]
pub struct Download {
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub binary_url: String,
}

#[derive(Deserialize, Clone)]
pub struct Modfile {
    #[serde(default)]
    pub id: u64,
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub filename: String,
    #[serde(default)]
    pub filesize: u64,
    #[serde(default)]
    pub date_added: i64,
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub version: String,
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub changelog: String,
    #[serde(default, deserialize_with = "deserialize_null_download")]
    pub download: Download,
}

fn deserialize_null_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

fn deserialize_null_download<'de, D>(deserializer: D) -> Result<Download, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Download>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

/// mod.io returns `modfile` as `null` (no published file) or occasionally as an
/// empty object `{}`. Treat both as "no file" so callers only ever see a real
/// modfile with a non-zero id.
fn deserialize_modfile<'de, D>(deserializer: D) -> Result<Option<Modfile>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let modfile = Option::<Modfile>::deserialize(deserializer)?;
    Ok(modfile.filter(|file| file.id != 0))
}

#[derive(Deserialize, Clone)]
pub struct ModObject {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub game_id: u64,
    #[serde(default)]
    pub visible: u8,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub profile_url: String,
    #[serde(default)]
    pub homepage_url: Option<String>,
    #[serde(default)]
    pub dependencies: bool,
    #[serde(default)]
    pub date_added: i64,
    #[serde(default)]
    pub date_updated: i64,
    #[serde(default)]
    pub date_live: i64,
    #[serde(default)]
    pub logo: Logo,
    #[serde(default)]
    pub submitted_by: UserObject,
    #[serde(default)]
    pub media: Media,
    #[serde(default)]
    pub tags: Vec<TagObject>,
    #[serde(default)]
    pub stats: ModStats,
    #[serde(default, deserialize_with = "deserialize_modfile")]
    pub modfile: Option<Modfile>,
}

#[derive(Deserialize)]
pub struct DependencyObject {
    #[serde(default)]
    pub mod_id: u64,
}

#[derive(Deserialize)]
pub struct GameTagOptionObject {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
pub struct AccessToken {
    #[serde(default)]
    pub access_token: String,
}

#[derive(Deserialize)]
pub struct Message {
    #[serde(default)]
    pub message: String,
}

/// Builder for the `GET /games/{id}/mods` query string.
#[derive(Default, Clone)]
pub struct ModQuery {
    pub search: Option<String>,
    /// AND-combined tag filters (`tags=<value>` repeated).
    pub tags: Vec<String>,
    /// OR-combined tag filter (`tags-in=a,b`).
    pub tags_in: Vec<String>,
    pub sort_field: Option<&'static str>,
    pub sort_desc: bool,
    pub limit: u32,
    pub offset: u32,
}

impl ModQuery {
    pub fn to_params(&self) -> Vec<(String, String)> {
        let mut params = Vec::new();
        if let Some(search) = &self.search {
            params.push(("_q".to_string(), search.clone()));
        }
        for tag in &self.tags {
            params.push(("tags".to_string(), tag.clone()));
        }
        if !self.tags_in.is_empty() {
            params.push(("tags-in".to_string(), self.tags_in.join(",")));
        }
        if let Some(field) = self.sort_field {
            let value = if self.sort_desc {
                format!("-{field}")
            } else {
                field.to_string()
            };
            params.push(("_sort".to_string(), value));
        }
        params.push(("_limit".to_string(), self.limit.to_string()));
        params.push(("_offset".to_string(), self.offset.to_string()));
        params
    }
}

pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    /// Earliest instant the next request may start. Guards both the steady-state
    /// pacing and the self-healing backoff after a `Retry-After` response.
    next_request_at: tokio::sync::Mutex<Instant>,
}

impl ApiClient {
    /// Builds a client, computing the API host the same way the previous `modio`
    /// crate did: test env, explicit host override, or the per-game host.
    pub fn new(
        api_key: String,
        game_id: Option<u64>,
        api_host: Option<&str>,
        use_test_env: bool,
    ) -> Result<Self, String> {
        let host = if use_test_env {
            TEST_HOST.to_string()
        } else if let Some(host) = api_host.filter(|host| !host.is_empty()) {
            host.to_string()
        } else if let Some(game_id) = game_id {
            format!("g-{game_id}.modapi.io")
        } else {
            DEFAULT_HOST.to_string()
        };

        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

        Ok(Self {
            http,
            base_url: format!("https://{host}/v1"),
            api_key,
            next_request_at: tokio::sync::Mutex::new(Instant::now()),
        })
    }

    /// Waits until this request is allowed to start, then reserves the next slot
    /// `MIN_REQUEST_INTERVAL` later. Concurrent callers chain correctly because
    /// the reservation happens under the lock before sleeping.
    async fn pace(&self) {
        let now = Instant::now();
        let wait = {
            let mut next = self.next_request_at.lock().await;
            let start = (*next).max(now);
            *next = start + MIN_REQUEST_INTERVAL;
            start.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    /// After a rate-limited response, hold off every subsequent request until the
    /// `Retry-After` window passes so we stop hammering mod.io during a block.
    async fn respect_retry_after(&self, retry_after_secs: u64) {
        let candidate = Instant::now() + Duration::from_secs(retry_after_secs);
        let mut next = self.next_request_at.lock().await;
        if candidate > *next {
            *next = candidate;
        }
    }

    async fn send_raw(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
        query: &[(String, String)],
        form: Option<&[(&str, &str)]>,
    ) -> Result<Vec<u8>, ApiError> {
        self.pace().await;

        let method_label = method.as_str().to_string();
        let url = format!("{}{}", self.base_url, path);
        let mut request = self
            .http
            .request(method, &url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header("X-Modio-Platform", PLATFORM_HEADER);

        let mut params: Vec<(String, String)> = query.to_vec();
        let authenticated = token.is_some();
        if let Some(token) = token {
            request = request.bearer_auth(token);
        } else {
            params.push(("api_key".to_string(), self.api_key.clone()));
        }
        request = request.query(&params);

        // mod.io requires `Content-Type: application/x-www-form-urlencoded` on
        // every write, even bodyless ones like subscribe/unsubscribe. `reqwest`
        // only sets it when a form body is present, so add it manually otherwise.
        if let Some(form) = form {
            request = request.form(form);
        } else if matches!(method_label.as_str(), "POST" | "PUT" | "DELETE") {
            request = request.header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            );
        }

        log_request(&method_label, &url, &params, authenticated, form);

        let started = Instant::now();
        let response = request.send().await.map_err(|e| {
            log::error!("mod.io <- {method_label} {path} transport error: {e}");
            ApiError::transport(format!("mod.io request failed: {e}"))
        })?;

        let status = response.status();
        log_response(&method_label, path, status, response.headers(), started.elapsed());

        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_retry_after_secs);

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ApiError::transport(format!("Failed to read mod.io response: {e}")))?
            .to_vec();

        if status.is_success() {
            return Ok(bytes);
        }

        let (error_ref, message) = match serde_json::from_slice::<ErrorEnvelope>(&bytes) {
            Ok(envelope) => (envelope.error.error_ref, envelope.error.message),
            Err(_) => (None, format!("mod.io returned HTTP {}", status.as_u16())),
        };
        let message = if message.is_empty() {
            format!("mod.io returned HTTP {}", status.as_u16())
        } else {
            message
        };

        let error = ApiError {
            status: Some(status.as_u16()),
            error_ref,
            message,
            retry_after_secs: retry_after,
        };
        // If mod.io rate limited us, pause all subsequent requests for the
        // advised window instead of immediately retrying into the same block.
        if error.is_rate_limited() {
            self.respect_retry_after(error.retry_after_secs.unwrap_or(60)).await;
        }
        error.log(path);
        Err(error)
    }

    async fn send<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
        query: &[(String, String)],
        form: Option<&[(&str, &str)]>,
    ) -> Result<T, ApiError> {
        let bytes = self.send_raw(method, path, token, query, form).await?;
        serde_json::from_slice(&bytes).map_err(|e| {
            ApiError::transport(format!("Failed to parse mod.io response: {e}"))
        })
    }

    async fn send_empty(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
    ) -> Result<(), ApiError> {
        self.send_raw(method, path, token, &[], None).await.map(|_| ())
    }

    /// When an api-key read fails, retry with OAuth unless mod.io is rate
    /// limiting us (a bearer retry would just consume more quota).
    fn should_retry_with_bearer(error: &ApiError) -> bool {
        !error.is_rate_limited()
    }

    pub async fn get_mods(
        &self,
        game_id: u64,
        query: &ModQuery,
    ) -> Result<ListResponse<ModObject>, ApiError> {
        let path = format!("/games/{game_id}/mods");
        self.send(reqwest::Method::GET, &path, None, &query.to_params(), None)
            .await
    }

    pub async fn get_mod(
        &self,
        game_id: u64,
        mod_id: u64,
        token: Option<&str>,
    ) -> Result<ModObject, ApiError> {
        let path = format!("/games/{game_id}/mods/{mod_id}");
        match self
            .send::<ModObject>(reqwest::Method::GET, &path, None, &[], None)
            .await
        {
            Ok(mod_) => Ok(mod_),
            Err(error) if token.is_some() && Self::should_retry_with_bearer(&error) => {
                log::debug!("get_mod {mod_id} failed with api key, retrying with bearer token");
                self.send(reqwest::Method::GET, &path, token, &[], None)
                    .await
            }
            Err(error) => Err(error),
        }
    }

    pub async fn get_mod_files(
        &self,
        game_id: u64,
        mod_id: u64,
        token: Option<&str>,
    ) -> Result<ListResponse<Modfile>, ApiError> {
        let path = format!("/games/{game_id}/mods/{mod_id}/files");
        let params = vec![
            ("_sort".to_string(), "-date_added".to_string()),
            ("_limit".to_string(), "100".to_string()),
        ];
        self.send(reqwest::Method::GET, &path, token, &params, None)
            .await
    }

    pub async fn get_mod_file(
        &self,
        game_id: u64,
        mod_id: u64,
        file_id: u64,
        token: Option<&str>,
    ) -> Result<Modfile, ApiError> {
        let path = format!("/games/{game_id}/mods/{mod_id}/files/{file_id}");
        self.send(reqwest::Method::GET, &path, token, &[], None)
            .await
    }

    pub async fn get_mod_dependencies(
        &self,
        game_id: u64,
        mod_id: u64,
        token: Option<&str>,
    ) -> Result<ListResponse<DependencyObject>, ApiError> {
        let path = format!("/games/{game_id}/mods/{mod_id}/dependencies");
        match self
            .send::<ListResponse<DependencyObject>>(reqwest::Method::GET, &path, None, &[], None)
            .await
        {
            Ok(list) => Ok(list),
            Err(error) if token.is_some() && Self::should_retry_with_bearer(&error) => {
                log::debug!(
                    "get_mod_dependencies {mod_id} failed with api key, retrying with bearer token"
                );
                self.send(reqwest::Method::GET, &path, token, &[], None)
                    .await
            }
            Err(error) => Err(error),
        }
    }

    pub async fn get_game_tags(
        &self,
        game_id: u64,
    ) -> Result<ListResponse<GameTagOptionObject>, ApiError> {
        let path = format!("/games/{game_id}/tags");
        self.send(reqwest::Method::GET, &path, None, &[], None).await
    }

    pub async fn get_user_subscriptions(
        &self,
        token: &str,
        game_id: u64,
        query: &ModQuery,
    ) -> Result<ListResponse<ModObject>, ApiError> {
        let mut params = query.to_params();
        params.push(("game_id".to_string(), game_id.to_string()));
        self.send(reqwest::Method::GET, "/me/subscribed", Some(token), &params, None)
            .await
    }

    pub async fn get_user_mods(
        &self,
        token: &str,
        game_id: u64,
        query: &ModQuery,
    ) -> Result<ListResponse<ModObject>, ApiError> {
        let mut params = query.to_params();
        params.push(("game_id".to_string(), game_id.to_string()));
        self.send(reqwest::Method::GET, "/me/mods", Some(token), &params, None)
            .await
    }

    pub async fn get_authenticated_user(&self, token: &str) -> Result<UserObject, ApiError> {
        self.send(reqwest::Method::GET, "/me", Some(token), &[], None)
            .await
    }

    pub async fn subscribe(&self, token: &str, game_id: u64, mod_id: u64) -> Result<(), ApiError> {
        let path = format!("/games/{game_id}/mods/{mod_id}/subscribe");
        self.send_empty(reqwest::Method::POST, &path, Some(token)).await
    }

    pub async fn unsubscribe(
        &self,
        token: &str,
        game_id: u64,
        mod_id: u64,
    ) -> Result<(), ApiError> {
        let path = format!("/games/{game_id}/mods/{mod_id}/subscribe");
        self.send_empty(reqwest::Method::DELETE, &path, Some(token))
            .await
    }

    pub async fn request_email_code(&self, email: &str) -> Result<Message, ApiError> {
        let form = [("email", email)];
        self.send(
            reqwest::Method::POST,
            "/oauth/emailrequest",
            None,
            &[],
            Some(&form),
        )
        .await
    }

    pub async fn exchange_email_code(&self, security_code: &str) -> Result<AccessToken, ApiError> {
        let form = [("security_code", security_code)];
        self.send(
            reqwest::Method::POST,
            "/oauth/emailexchange",
            None,
            &[],
            Some(&form),
        )
        .await
    }
}

/// Parses the `Retry-After` header. mod.io returns whole seconds.
fn parse_retry_after_secs(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

/// Logs an outgoing mod.io request, redacting secrets (`api_key`, bearer token)
/// while still showing method, full query and the headers we attach.
fn log_request(
    method: &str,
    url: &str,
    params: &[(String, String)],
    authenticated: bool,
    form: Option<&[(&str, &str)]>,
) {
    let query: Vec<String> = params
        .iter()
        .map(|(key, value)| {
            if key == "api_key" {
                format!("{key}=***")
            } else {
                format!("{key}={value}")
            }
        })
        .collect();
    let query_string = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query.join("&"))
    };

    let mut headers = vec![
        "Accept: application/json".to_string(),
        format!("X-Modio-Platform: {PLATFORM_HEADER}"),
        format!("User-Agent: {USER_AGENT}"),
    ];
    if authenticated {
        headers.push("Authorization: Bearer ***".to_string());
    }
    if form.is_some() || matches!(method, "POST" | "PUT" | "DELETE") {
        headers.push("Content-Type: application/x-www-form-urlencoded".to_string());
    }
    let body = form.map(|fields| {
        let pairs: Vec<String> = fields
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        format!(" body=[{}]", pairs.join(", "))
    });

    log::info!(
        "mod.io -> {method} {url}{query_string} (auth={}) headers=[{}]{}",
        if authenticated { "bearer" } else { "api_key" },
        headers.join(", "),
        body.unwrap_or_default()
    );
}

/// Logs the response status, elapsed time and all response headers.
fn log_response(
    method: &str,
    path: &str,
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    elapsed: std::time::Duration,
) {
    let header_dump: Vec<String> = headers
        .iter()
        .map(|(name, value)| format!("{name}: {}", value.to_str().unwrap_or("<binary>")))
        .collect();
    let elapsed_ms = elapsed.as_millis();
    let line = format!(
        "mod.io <- {} {method} {path} ({elapsed_ms}ms) headers=[{}]",
        status.as_u16(),
        header_dump.join(", ")
    );
    if status.is_success() {
        log::info!("{line}");
    } else {
        log::warn!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_query_serializes_all_params() {
        let query = ModQuery {
            search: Some("rogue".to_string()),
            tags: vec!["Plugin".to_string()],
            tags_in: vec!["A".to_string(), "B".to_string()],
            sort_field: Some("downloads_total"),
            sort_desc: true,
            limit: 20,
            offset: 40,
        };
        assert_eq!(
            query.to_params(),
            vec![
                ("_q".to_string(), "rogue".to_string()),
                ("tags".to_string(), "Plugin".to_string()),
                ("tags-in".to_string(), "A,B".to_string()),
                ("_sort".to_string(), "-downloads_total".to_string()),
                ("_limit".to_string(), "20".to_string()),
                ("_offset".to_string(), "40".to_string()),
            ]
        );
    }

    #[test]
    fn mod_query_ascending_sort_has_no_prefix() {
        let query = ModQuery {
            sort_field: Some("name"),
            sort_desc: false,
            limit: 10,
            offset: 0,
            ..Default::default()
        };
        let params = query.to_params();
        assert!(params.contains(&("_sort".to_string(), "name".to_string())));
    }

    #[test]
    fn modfile_null_deserializes_to_none() {
        let mod_: ModObject = serde_json::from_str(r#"{"id":1,"modfile":null}"#).unwrap();
        assert!(mod_.modfile.is_none());
    }

    #[test]
    fn empty_modfile_object_deserializes_to_none() {
        let mod_: ModObject = serde_json::from_str(r#"{"id":1,"modfile":{}}"#).unwrap();
        assert!(mod_.modfile.is_none());
    }

    #[test]
    fn populated_modfile_deserializes_to_some() {
        let mod_: ModObject = serde_json::from_str(
            r#"{"id":1,"modfile":{"id":42,"filename":"a.zip","filesize":10,"download":{"binary_url":"https://example.com/a.zip"}}}"#,
        )
        .unwrap();
        let file = mod_.modfile.expect("modfile should be present");
        assert_eq!(file.id, 42);
        assert_eq!(file.filesize, 10);
        assert_eq!(file.download.binary_url, "https://example.com/a.zip");
    }

    #[test]
    fn modfile_null_string_fields_deserialize_to_empty() {
        let file: Modfile = serde_json::from_str(
            r#"{"id":7,"filename":"a.zip","version":null,"changelog":null,"download":{"binary_url":null}}"#,
        )
        .unwrap();
        assert_eq!(file.version, "");
        assert_eq!(file.changelog, "");
        assert_eq!(file.download.binary_url, "");
    }

    #[test]
    fn modfile_list_response_tolerates_null_fields() {
        let list: ListResponse<Modfile> = serde_json::from_str(
            r#"{"data":[{"id":1,"filename":"one.zip","version":null,"changelog":null,"download":null},{"id":2,"filename":"two.zip","version":"2.0","changelog":"fixes","download":{"binary_url":"https://example.com/two.zip"}}]}"#,
        )
        .unwrap();
        assert_eq!(list.data.len(), 2);
        assert_eq!(list.data[0].version, "");
        assert_eq!(list.data[1].version, "2.0");
    }

    #[test]
    fn parses_retry_after_seconds() {
        assert_eq!(parse_retry_after_secs("60"), Some(60));
        assert_eq!(parse_retry_after_secs(" 52 "), Some(52));
        assert_eq!(parse_retry_after_secs("soon"), None);
    }

    #[test]
    fn rate_limit_classification() {
        let err = ApiError {
            status: Some(429),
            error_ref: Some(11009),
            message: "slow down".to_string(),
            retry_after_secs: Some(60),
        };
        assert!(err.is_rate_limited());
        assert!(!err.is_auth());
    }

    #[test]
    fn not_subscribed_classification() {
        let err = ApiError {
            status: Some(400),
            error_ref: Some(15005),
            message: "not subscribed".to_string(),
            retry_after_secs: None,
        };
        assert!(err.is_not_subscribed());
        assert!(!err.is_not_found());
        assert!(!err.is_rate_limited());
    }
}
