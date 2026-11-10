//! Behavior of `ApiClient` when mod.io rejects credentials (issue #45: a burst
//! of 401s for every request after the session token expired and the game
//! api_key was refused). Runs the real client against a local fake mod.io and
//! observes what reaches the server and what would be reported to Sentry
//! (see `logging::report_capture`).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use super::*;
use crate::logging::report_capture::{capture_reports, sentry_reports};

pub(crate) const GAME_ID: u64 = 3213;
pub(crate) const TOKEN: &str = "expired-token";

#[derive(Clone, Debug)]
pub(crate) struct Received {
    pub(crate) path: String,
    pub(crate) bearer: bool,
}

/// Per-response headers real mod.io sends (through Cloudflare), so tests can
/// check they stay out of Sentry.
pub(crate) const FAKE_DATE: &str = "Tue, 10 Nov 2026 12:34:56 GMT";
pub(crate) const FAKE_COOKIE: &str = "__cf_bm=fake-cloudflare-cookie; path=/; HttpOnly";

type Responder = dyn Fn(&Received) -> (u16, Option<u32>, String) + Send + Sync;

/// Minimal HTTP/1.1 server: one response per connection, records every request.
pub(crate) struct FakeModio {
    pub(crate) base_url: String,
    received: Arc<Mutex<Vec<Received>>>,
}

impl FakeModio {
    pub(crate) fn start(respond: impl Fn(&Received) -> (u16, Option<u32>, String) + Send + Sync + 'static) -> Self {
        Self::start_with(None, respond)
    }

    /// Like [`FakeModio::start`], but every 429 response carries
    /// `Retry-After: {retry_after_secs}`, as mod.io sends it.
    pub(crate) fn start_with_retry_after(
        retry_after_secs: u64,
        respond: impl Fn(&Received) -> (u16, Option<u32>, String) + Send + Sync + 'static,
    ) -> Self {
        Self::start_with(Some(retry_after_secs), respond)
    }

    fn start_with(
        retry_after_secs: Option<u64>,
        respond: impl Fn(&Received) -> (u16, Option<u32>, String) + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = received.clone();
        let respond: Arc<Responder> = Arc::new(respond);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                let target = request_line.split_whitespace().nth(1).unwrap_or("").to_string();
                let path = target
                    .strip_prefix("/v1")
                    .unwrap_or(&target)
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .to_string();
                let mut bearer = false;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if line.to_ascii_lowercase().starts_with("authorization: bearer") {
                        bearer = true;
                    }
                }
                let request = Received { path, bearer };
                log.lock().unwrap().push(request.clone());
                let (status, error_ref, body) = respond(&request);
                let mut response = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\ndate: {FAKE_DATE}\r\nset-cookie: {FAKE_COOKIE}\r\n",
                    body.len()
                );
                if let Some(error_ref) = error_ref {
                    response.push_str(&format!("x-modio-error-ref: {error_ref}\r\n"));
                }
                if let Some(retry_after_secs) = retry_after_secs.filter(|_| status == 429) {
                    response.push_str(&format!("retry-after: {retry_after_secs}\r\n"));
                }
                response.push_str("\r\n");
                response.push_str(&body);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self { base_url, received }
    }

    /// A client whose api-key-refused hook counts its calls and whose
    /// token-rejected hook records tokens, like the hooks
    /// `ModioState::install_api_client_hooks` installs in the app.
    fn client_counting_api_key_refusals(&self) -> (ApiClient, Arc<Mutex<u32>>) {
        let refusals = Arc::new(Mutex::new(0));
        let sink = refusals.clone();
        let (on_token_rejected, _) = recording_token_hook();
        let client = self.client_with_hooks(ApiClientHooks {
            on_token_rejected: Some(on_token_rejected),
            on_api_key_refused: Some(Arc::new(move || {
                *sink.lock().unwrap() += 1;
            })),
        });
        (client, refusals)
    }

    pub(crate) fn received(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }

    /// A client whose token-rejected hook records every rejected token, like
    /// the hooks `ModioState::install_api_client_hooks` installs in the app.
    fn client(&self) -> (ApiClient, Arc<Mutex<Vec<String>>>) {
        let (on_token_rejected, rejected) = recording_token_hook();
        let client = self.client_with_hooks(ApiClientHooks {
            on_token_rejected: Some(on_token_rejected),
            ..ApiClientHooks::default()
        });
        (client, rejected)
    }

    fn client_with_hooks(&self, hooks: ApiClientHooks) -> ApiClient {
        ApiClient::new("game-key".into(), Some(GAME_ID), None, false)
            .unwrap()
            .with_base_url(self.base_url.clone())
            .with_hooks(hooks)
    }
}

/// A token-rejected hook that records every rejected token it is called with.
fn recording_token_hook() -> (TokenRejectedHook, Arc<Mutex<Vec<String>>>) {
    let rejected = Arc::new(Mutex::new(Vec::new()));
    let sink = rejected.clone();
    let hook: TokenRejectedHook = Arc::new(move |token: &str| {
        sink.lock().unwrap().push(token.to_string());
    });
    (hook, rejected)
}

pub(crate) fn error_body(error_ref: u32, message: &str) -> String {
    format!(r#"{{"error":{{"code":401,"error_ref":{error_ref},"message":"{message}"}}}}"#)
}

pub(crate) fn token_expired() -> (u16, Option<u32>, String) {
    (401, Some(TOKEN_REJECTED_ERROR_REF), error_body(TOKEN_REJECTED_ERROR_REF, "Access token is expired, or has been revoked."))
}

// --- Behavior ----------------------------------------------------------------

#[tokio::test]
async fn a_rejected_token_is_not_sent_to_mod_io_again() {
    let server = FakeModio::start(|_| token_expired());
    let (client, rejected) = server.client();

    let first = client.get_user_subscriptions(TOKEN, GAME_ID, &ModQuery::default()).await.err().expect("request should fail");
    let second = client.get_user_subscriptions(TOKEN, GAME_ID, &ModQuery::default()).await.err().expect("request should fail");

    assert!(first.is_unauthorized());
    assert!(second.is_unauthorized());
    assert_eq!(server.received().len(), 1, "second request should not reach mod.io");
    assert_eq!(*rejected.lock().unwrap(), vec![TOKEN.to_string()]);
}

/// A private mod: the api_key read is refused, so the client retries with the
/// session token, which mod.io rejects as expired.
fn private_mod_with_expired_token(request: &Received) -> (u16, Option<u32>, String) {
    if request.bearer {
        token_expired()
    } else {
        (403, None, r#"{"error":{"code":403,"message":"Forbidden"}}"#.to_string())
    }
}

#[tokio::test]
async fn an_expired_token_on_a_mod_read_retry_is_not_reported_to_sentry() {
    capture_reports();
    let server = FakeModio::start(private_mod_with_expired_token);
    let (client, rejected) = server.client();

    let mod_error = client.get_mod(GAME_ID, ModId(2400035), Some(TOKEN)).await.err().expect("request should fail");
    let deps_error = client
        .get_mod_dependencies(GAME_ID, ModId(2400035), Some(TOKEN))
        .await
        .err()
        .expect("request should fail");

    assert!(mod_error.is_unauthorized());
    assert!(deps_error.is_unauthorized());
    let bearer_requests = server.received().iter().filter(|request| request.bearer).count();
    assert_eq!(bearer_requests, 1, "the rejected token should not be retried on the next read");
    assert_eq!(*rejected.lock().unwrap(), vec![TOKEN.to_string()]);
    assert_eq!(sentry_reports(), Vec::<String>::new());
}

/// mod.io `error_ref` for "api_key supplied is invalid".
const API_KEY_INVALID_ERROR_REF: u32 = 11002;

fn api_key_invalid() -> (u16, Option<u32>, String) {
    (401, Some(API_KEY_INVALID_ERROR_REF), error_body(API_KEY_INVALID_ERROR_REF, "api_key supplied is invalid."))
}

#[tokio::test]
async fn a_refused_api_key_is_reported_to_sentry_once() {
    capture_reports();
    let server = FakeModio::start(|_| api_key_invalid());
    let (client, rejected) = server.client();

    let tags = client.get_game_tags(GAME_ID).await.err().expect("request should fail");
    let mods = client
        .get_mods(GAME_ID, &ModQuery::default())
        .await
        .err()
        .expect("request should fail");
    let tags_again = client.get_game_tags(GAME_ID).await.err().expect("request should fail");

    assert!(tags.is_unauthorized() && mods.is_unauthorized() && tags_again.is_unauthorized());
    let paths: Vec<String> = server.received().into_iter().map(|request| request.path).collect();
    assert_eq!(paths, ["/games/3213/tags", "/games/3213/mods", "/games/3213/tags"]);
    assert!(rejected.lock().unwrap().is_empty(), "no session token was involved");
    let reports = sentry_reports();
    assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
    assert!(reports[0].contains("game API key refused"), "report should name the cause: {}", reports[0]);
}

#[tokio::test]
async fn requests_queued_behind_the_rejection_are_not_sent() {
    let server = FakeModio::start(|_| token_expired());
    let (client, _) = server.client();

    let query = ModQuery::default();
    let (subscribed, own_mods, me) = tokio::join!(
        client.get_user_subscriptions(TOKEN, GAME_ID, &query),
        client.get_user_mods(TOKEN, GAME_ID, &query),
        client.get_authenticated_user(TOKEN),
    );

    assert!(subscribed.is_err() && own_mods.is_err() && me.is_err());
    assert_eq!(server.received().len(), 1, "only the first request should reach mod.io");
}

#[tokio::test]
async fn a_refused_api_key_signals_the_app_once() {
    let server = FakeModio::start(|_| api_key_invalid());
    let (client, refusals) = server.client_counting_api_key_refusals();

    let _ = client.get_game_tags(GAME_ID).await;
    let _ = client.get_mods(GAME_ID, &ModQuery::default()).await;
    let _ = client.get_game_tags(GAME_ID).await;

    assert_eq!(*refusals.lock().unwrap(), 1);
}

#[tokio::test]
async fn an_expired_token_does_not_signal_a_refused_api_key() {
    let server = FakeModio::start(private_mod_with_expired_token);
    let (client, refusals) = server.client_counting_api_key_refusals();

    let _ = client.get_mod(GAME_ID, ModId(2400035), Some(TOKEN)).await;
    let _ = client.get_user_subscriptions(TOKEN, GAME_ID, &ModQuery::default()).await;

    assert_eq!(*refusals.lock().unwrap(), 0);
}

// --- Private mods read without a session token (issue #68) -------------------

pub(crate) fn forbidden() -> (u16, Option<u32>, String) {
    (403, None, r#"{"error":{"code":403,"message":"Forbidden"}}"#.to_string())
}

#[tokio::test]
async fn a_private_mod_read_without_a_token_is_not_reported_to_sentry() {
    capture_reports();
    let server = FakeModio::start(|_| forbidden());
    let (client, _) = server.client();

    let error = client.get_mod(GAME_ID, ModId(2400035), None).await.err().expect("request should fail");

    assert_eq!(error.status, Some(403));
    assert_eq!(sentry_reports(), Vec::<String>::new());
}

#[tokio::test]
async fn a_private_mod_dependencies_read_without_a_token_is_not_reported_to_sentry() {
    capture_reports();
    let server = FakeModio::start(|_| forbidden());
    let (client, _) = server.client();

    let error = client
        .get_mod_dependencies(GAME_ID, ModId(2400035), None)
        .await
        .err()
        .expect("request should fail");

    assert_eq!(error.status, Some(403));
    assert_eq!(sentry_reports(), Vec::<String>::new());
}

#[tokio::test]
async fn a_server_error_on_a_mod_read_without_a_token_is_still_reported_to_sentry() {
    capture_reports();
    let server = FakeModio::start(|_| (503, None, r#"{"error":{"code":503,"message":"Service Unavailable"}}"#.to_string()));
    let (client, _) = server.client();

    let mod_error = client.get_mod(GAME_ID, ModId(2400035), None).await.err().expect("request should fail");
    let deps_error = client
        .get_mod_dependencies(GAME_ID, ModId(2400035), None)
        .await
        .err()
        .expect("request should fail");

    assert!(mod_error.is_server_error() && deps_error.is_server_error());
    let reports = sentry_reports();
    for path in ["/games/{game}/mods/{mod} ", "/games/{game}/mods/{mod}/dependencies"] {
        assert!(reports.iter().any(|report| report.contains(path)), "no report for {path}: {reports:#?}");
    }
}

// --- A whole session refused by mod.io (issue #74) ---------------------------

const EMAIL: &str = "player@example.com";

/// mod.io refuses both credentials: the game api_key (11002) and the session
/// token (11005), as in the sessions behind #74 and its duplicates.
fn every_credential_refused(request: &Received) -> (u16, Option<u32>, String) {
    if request.bearer {
        token_expired()
    } else {
        api_key_invalid()
    }
}

/// Every request from the #74 cluster, in the order a session makes them.
async fn make_every_request_a_refused_session_made(client: &ApiClient) {
    let query = ModQuery::default();
    let _ = client.get_authenticated_user(TOKEN).await;
    let _ = client.get_user_mods(TOKEN, GAME_ID, &query).await;
    let _ = client.get_user_subscriptions(TOKEN, GAME_ID, &query).await;
    let _ = client.get_mods(GAME_ID, &query).await;
    let _ = client.get_game_tags(GAME_ID).await;
    for mod_id in [ModId(2400035), ModId(6315836), ModId(3137985)] {
        let _ = client.get_mod(GAME_ID, mod_id, Some(TOKEN)).await;
        let _ = client.get_mod_dependencies(GAME_ID, mod_id, Some(TOKEN)).await;
    }
    let _ = client.request_email_code(EMAIL).await;
}

#[tokio::test]
async fn a_session_refused_on_every_request_is_reported_to_sentry_once() {
    capture_reports();
    let server = FakeModio::start(every_credential_refused);
    let (client, rejected) = server.client();

    make_every_request_a_refused_session_made(&client).await;

    assert_eq!(*rejected.lock().unwrap(), vec![TOKEN.to_string()], "the token should be rejected once");
    let reports = sentry_reports();
    assert_eq!(reports.len(), 1, "expected one report for the whole session, got {reports:#?}");
    assert!(reports[0].contains("game API key refused"), "the one report should be the API key: {}", reports[0]);
}

/// Sentry groups message events by their text, so the report for a refused
/// API key must not depend on which request happened to be refused first.
#[tokio::test]
async fn a_refused_api_key_is_one_sentry_issue_whichever_request_is_refused_first() {
    let server = FakeModio::start(|_| api_key_invalid());

    capture_reports();
    let (first_run, _) = server.client();
    let _ = first_run.get_game_tags(GAME_ID).await;
    let tags_first = sentry_reports();

    capture_reports();
    let (second_run, _) = server.client();
    let _ = second_run.request_email_code(EMAIL).await;
    let email_first = sentry_reports();

    assert_eq!(tags_first.len(), 1, "{tags_first:#?}");
    assert_eq!(tags_first, email_first, "both runs should file the same Sentry issue");
}

#[tokio::test]
async fn a_refused_api_key_is_one_sentry_issue_however_mod_io_words_it() {
    capture_reports();
    let server = FakeModio::start(|_| api_key_invalid());
    let reworded = FakeModio::start(|_| {
        (401, Some(API_KEY_INVALID_ERROR_REF), error_body(API_KEY_INVALID_ERROR_REF, "The api_key you supplied is not valid."))
    });

    let (first_run, _) = server.client();
    let _ = first_run.get_game_tags(GAME_ID).await;
    let original = sentry_reports();

    capture_reports();
    let (second_run, _) = reworded.client();
    let _ = second_run.get_game_tags(GAME_ID).await;

    assert_eq!(original.len(), 1, "{original:#?}");
    assert_eq!(original, sentry_reports(), "a reworded refusal should file the same Sentry issue");
}

// --- Tokenless 401s that are not a refused API key (issue #122) ---------------

/// mod.io `error_ref` for "the security code is not valid" (`/oauth/emailexchange`).
const SECURITY_CODE_INVALID_ERROR_REF: u32 = 11014;

fn security_code_invalid() -> (u16, Option<u32>, String) {
    (401, Some(SECURITY_CODE_INVALID_ERROR_REF), error_body(SECURITY_CODE_INVALID_ERROR_REF, "The security code is invalid."))
}

/// The email code is refused first; the game API key is refused later in the run.
fn email_code_refused_then_api_key_refused(request: &Received) -> (u16, Option<u32>, String) {
    if request.path == "/oauth/emailexchange" {
        security_code_invalid()
    } else {
        api_key_invalid()
    }
}

#[tokio::test]
async fn a_refused_api_key_is_reported_once_after_another_tokenless_401() {
    capture_reports();
    let server = FakeModio::start(email_code_refused_then_api_key_refused);
    let (client, refusals) = server.client_counting_api_key_refusals();

    let code = client.exchange_email_code("12345").await.err().expect("request should fail");
    let tags = client.get_game_tags(GAME_ID).await.err().expect("request should fail");

    assert!(code.is_unauthorized(), "the email code should be refused: {code:?}");
    assert!(tags.is_api_key_refused(), "the API key should be refused: {tags:?}");
    let refused: Vec<String> = sentry_reports()
        .into_iter()
        .filter(|report| report.contains("game API key refused"))
        .collect();
    assert_eq!(refused.len(), 1, "expected one refused-key report, got {refused:#?}");
    let api_key_invalid = format!("error_ref=Some({API_KEY_INVALID_ERROR_REF})");
    assert!(refused[0].contains(&api_key_invalid), "the report should be the 11002: {}", refused[0]);
    assert_eq!(*refusals.lock().unwrap(), 1);
}

#[tokio::test]
async fn a_wrong_or_expired_email_code_is_not_reported_to_sentry() {
    // mod.io's 401s for a code that is already used, expired, invalid or used up.
    for error_ref in [11011, 11012, SECURITY_CODE_INVALID_ERROR_REF, 11147] {
        capture_reports();
        let server = FakeModio::start(move |_| (401, Some(error_ref), error_body(error_ref, "Security code refused.")));
        let (client, refusals) = server.client_counting_api_key_refusals();

        let error = client.exchange_email_code("12345").await.err().expect("request should fail");

        assert!(error.is_unauthorized());
        assert_eq!(sentry_reports(), Vec::<String>::new(), "error_ref {error_ref}");
        assert_eq!(*refusals.lock().unwrap(), 0, "error_ref {error_ref}");
    }
}

/// The `error_ref` in the body is what counts: mod.io may leave out the
/// `x-modio-error-ref` header.
#[tokio::test]
async fn a_refused_api_key_without_the_error_ref_header_is_reported_once() {
    capture_reports();
    let server = FakeModio::start(|_| {
        (401, None, error_body(API_KEY_INVALID_ERROR_REF, "api_key supplied is invalid."))
    });
    let (client, _) = server.client();

    let _ = client.get_game_tags(GAME_ID).await;
    let _ = client.get_mods(GAME_ID, &ModQuery::default()).await;

    let reports = sentry_reports();
    assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
}

/// mod.io `error_ref` for "the api_key differs from the one that requested the
/// security code": an app bug, not the user's doing.
const SECURITY_CODE_DIFFERENT_KEY_ERROR_REF: u32 = 11013;

#[tokio::test]
async fn another_tokenless_401_is_reported_as_itself_and_leaves_the_refused_key_report() {
    capture_reports();
    let server = FakeModio::start(|request| {
        if request.path == "/oauth/emailexchange" {
            let error_ref = SECURITY_CODE_DIFFERENT_KEY_ERROR_REF;
            (401, Some(error_ref), error_body(error_ref, "The api_key differs."))
        } else {
            api_key_invalid()
        }
    });
    let (client, refusals) = server.client_counting_api_key_refusals();

    let _ = client.exchange_email_code("12345").await;
    let _ = client.get_game_tags(GAME_ID).await;

    let reports = sentry_reports();
    let different_key = format!(
        "mod.io API error: POST /oauth/emailexchange returned status=401, error_ref=Some({SECURITY_CODE_DIFFERENT_KEY_ERROR_REF})"
    );
    assert!(
        reports.iter().any(|report| report.contains(&different_key)),
        "the 11013 should be reported as itself: {reports:#?}"
    );
    let refused_key = format!("game API key refused, later refusals logged at info): status=Some(401), error_ref=Some({API_KEY_INVALID_ERROR_REF})");
    assert_eq!(
        reports.iter().filter(|report| report.contains(&refused_key)).count(),
        1,
        "the refused key should be reported once: {reports:#?}"
    );
    assert_eq!(*refusals.lock().unwrap(), 1);
}

// --- Stable Sentry text for failed requests (issue #121) ---------------------

fn service_unavailable() -> (u16, Option<u32>, String) {
    (503, None, r#"{"error":{"code":503,"message":"Service Unavailable"}}"#.to_string())
}

#[tokio::test]
async fn a_failed_request_reports_no_response_headers() {
    capture_reports();
    let server = FakeModio::start(|_| service_unavailable());
    let (client, _) = server.client();

    let _ = client.get_mod_file(GAME_ID, ModId(4406867), FileId(6642350), None).await;

    let reports = sentry_reports();
    assert!(!reports.is_empty(), "the failure should still be reported");
    for report in &reports {
        assert!(!report.contains(FAKE_DATE), "report contains the date header: {report}");
        assert!(!report.contains(FAKE_COOKIE), "report contains the cookie: {report}");
    }
}

/// Sentry groups message events by their text, so the same failure on
/// different mods must be one issue, not one per mod.
#[tokio::test]
async fn the_same_failure_on_different_mods_is_reported_with_the_same_text() {
    let server = FakeModio::start(|_| service_unavailable());
    let (client, _) = server.client();

    capture_reports();
    let _ = client.get_mod_file(GAME_ID, ModId(4406867), FileId(6642350), None).await;
    let first = sentry_reports();

    capture_reports();
    let _ = client.get_mod_file(GAME_ID, ModId(1), FileId(2), None).await;
    let second = sentry_reports();

    assert_eq!(first.len(), 1, "{first:#?}");
    assert_eq!(first, second, "both failures should file the same Sentry issue");
    assert!(first[0].contains("GET /games/{game}/mods/{mod}/files/{file} "), "{}", first[0]);
    assert!(first[0].contains("503"), "the report should keep the status: {}", first[0]);
}

/// A private mod read with a session token while mod.io rate limits: the
/// api_key read is refused, and the bearer retry gets a 429.
fn private_mod_rate_limited_on_retry(request: &Received) -> (u16, Option<u32>, String) {
    if request.bearer {
        (429, None, r#"{"error":{"code":429,"message":"Too Many Requests"}}"#.to_string())
    } else {
        forbidden()
    }
}

#[tokio::test]
async fn a_rate_limited_bearer_retry_is_reported_once() {
    capture_reports();
    let server = FakeModio::start(private_mod_rate_limited_on_retry);
    let (client, _) = server.client();

    let error = client.get_mod(GAME_ID, ModId(2400035), Some(TOKEN)).await.err().expect("request should fail");

    assert!(error.is_rate_limited());
    let reports = sentry_reports();
    assert_eq!(reports.len(), 1, "expected one report for the failed read, got {reports:#?}");
    assert!(reports[0].contains("429"), "the report should be the rate limit: {}", reports[0]);
}

/// A base URL nothing listens on, so every request fails before a response.
fn unreachable_base_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    drop(listener);
    base_url
}

#[tokio::test]
async fn a_transport_failure_is_reported_with_the_same_text_for_every_mod() {
    let client = ApiClient::new("game-key".into(), Some(GAME_ID), None, false)
        .unwrap()
        .with_base_url(unreachable_base_url());

    capture_reports();
    let _ = client.get_mod_file(GAME_ID, ModId(4406867), FileId(6642350), None).await;
    let first = sentry_reports();

    capture_reports();
    let _ = client.get_mod_file(GAME_ID, ModId(1), FileId(2), None).await;
    let second = sentry_reports();

    assert_eq!(first.len(), 1, "{first:#?}");
    assert_eq!(first, second, "both failures should file the same Sentry issue");
    assert!(!first[0].contains("game-key"), "the report contains the API key: {}", first[0]);
}
