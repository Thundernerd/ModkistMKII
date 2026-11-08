//! What callers log when mod.io rejects our credentials mid-operation (issue
//! #59). `ApiClient` already handles and reports a rejected session token or
//! refused game API key once; the callers must not turn it into one Sentry
//! report per mod. Runs the real callers against a local fake mod.io and
//! captures the `warn`/`error` records `SentryLogWriter` would forward.

use std::collections::HashSet;

use super::*;
use crate::modio_api::auth_tests::{
    capture_reports, sentry_reports, token_expired, FakeModio, Received, TOKEN,
};

const PRIVATE_MODS: [u64; 3] = [101, 102, 103];

/// Private mods: an api_key read is refused (403), and the bearer retry is
/// rejected because the session token expired.
fn private_mods_with_expired_token(request: &Received) -> (u16, Option<u32>, String) {
    if request.bearer {
        token_expired()
    } else {
        (403, None, r#"{"error":{"code":403,"message":"Forbidden"}}"#.to_string())
    }
}

fn bearer_requests(server: &FakeModio) -> usize {
    server.received().iter().filter(|request| request.bearer).count()
}

#[tokio::test]
async fn an_expired_token_while_mapping_dependencies_is_not_reported_per_mod() {
    capture_reports();
    let server = FakeModio::start(private_mods_with_expired_token);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);

    let installed: HashSet<u64> = PRIVATE_MODS.into_iter().collect();
    let map = build_dependency_map(&state, &installed).await;

    assert_eq!(sentry_reports(), Vec::<String>::new());
    assert!(state.auth_status().session_expired);
    assert_eq!(map.len(), PRIVATE_MODS.len());
    assert!(map.values().all(Vec::is_empty));
    assert_eq!(
        server.received().len(),
        2,
        "only the first mod should be read before the session ended: {:#?}",
        server.received()
    );
    assert_eq!(bearer_requests(&server), 1);
}

fn server_error(_: &Received) -> (u16, Option<u32>, String) {
    (503, None, r#"{"error":{"code":503,"message":"Service Unavailable"}}"#.to_string())
}

#[tokio::test]
async fn a_server_error_while_mapping_dependencies_is_still_reported() {
    capture_reports();
    let server = FakeModio::start(server_error);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);

    let installed: HashSet<u64> = PRIVATE_MODS.into_iter().collect();
    build_dependency_map(&state, &installed).await;

    let caller_warnings: Vec<String> = sentry_reports()
        .into_iter()
        .filter(|report| report.starts_with("WARN: Building installed mod dependency map"))
        .collect();
    assert_eq!(caller_warnings.len(), PRIVATE_MODS.len(), "{caller_warnings:#?}");
    assert!(!state.auth_status().session_expired);
}

const EMPTY_LIST: &str = r#"{"data":[],"result_total":0}"#;

/// The `id-in` batch read only returns public mods, so every private mod falls
/// back to its own read.
fn batch_omits_private_mods_with_expired_token(request: &Received) -> (u16, Option<u32>, String) {
    if request.path == "/games/3213/mods" {
        (200, None, EMPTY_LIST.to_string())
    } else {
        private_mods_with_expired_token(request)
    }
}

fn failed_on_rejected_credentials(outcome: Option<&ModFetchOutcome>) -> bool {
    matches!(outcome, Some(ModFetchOutcome::Failed(error)) if error.credentials_rejected)
}

#[tokio::test]
async fn an_expired_token_while_fetching_mods_stops_the_per_mod_fallback() {
    capture_reports();
    let server = FakeModio::start(batch_omits_private_mods_with_expired_token);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);

    let outcomes = fetch_mod_outcomes_batch(&state, &PRIVATE_MODS).await;

    assert_eq!(sentry_reports(), Vec::<String>::new());
    assert!(PRIVATE_MODS
        .iter()
        .all(|mod_id| failed_on_rejected_credentials(outcomes.get(mod_id))));
    let paths: Vec<String> = server.received().into_iter().map(|request| request.path).collect();
    assert_eq!(paths, ["/games/3213/mods", "/games/3213/mods/101", "/games/3213/mods/101"]);
}
