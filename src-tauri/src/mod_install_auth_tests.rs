//! What callers log when mod.io answers with an expected failure: rejected
//! credentials mid-operation (issue #59), private mods while signed out (#68)
//! and mod files the author deleted (#75). `ApiClient` already handles and
//! reports these; the callers must not turn them into one Sentry report per
//! mod. Runs the real callers against a local fake mod.io and captures the
//! `warn`/`error` records `SentryLogWriter` would forward.

use std::collections::HashSet;

use super::*;
use crate::logging::report_capture::{capture_reports, sentry_reports};
use crate::modio_api::auth_tests::{
    error_body, forbidden, token_expired, FakeModio, Received, GAME_ID, TOKEN,
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
    if request.path == format!("/games/{GAME_ID}/mods") {
        (200, None, EMPTY_LIST.to_string())
    } else {
        private_mods_with_expired_token(request)
    }
}

fn failed_on_rejected_credentials(outcome: Option<&ModFetchOutcome>) -> bool {
    matches!(outcome, Some(ModFetchOutcome::Failed(error)) if error.credentials_rejected())
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
    let mods = format!("/games/{GAME_ID}/mods");
    assert_eq!(paths, [mods.clone(), format!("{mods}/101"), format!("{mods}/101")]);
}

/// mod.io `error_ref` for "api_key supplied is invalid".
const API_KEY_INVALID_ERROR_REF: u32 = 11002;

/// mod.io refuses the game API key; the session token still works, but these
/// mods were deleted, so the bearer read finds nothing either.
fn api_key_refused_and_mods_gone(request: &Received) -> (u16, Option<u32>, String) {
    if request.bearer {
        (404, None, r#"{"error":{"code":404,"message":"Not Found"}}"#.to_string())
    } else {
        let body = error_body(API_KEY_INVALID_ERROR_REF, "api_key supplied is invalid.");
        (401, Some(API_KEY_INVALID_ERROR_REF), body)
    }
}

#[tokio::test]
async fn a_refused_api_key_on_the_batch_mod_fetch_is_reported_once() {
    capture_reports();
    let server = FakeModio::start(api_key_refused_and_mods_gone);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);

    let outcomes = fetch_mod_outcomes_batch(&state, &PRIVATE_MODS).await;

    let reports = sentry_reports();
    assert_eq!(reports.len(), 1, "only the client's own report: {reports:#?}");
    assert!(reports[0].contains("game API key refused"), "{}", reports[0]);
    assert!(PRIVATE_MODS
        .iter()
        .all(|mod_id| matches!(outcomes.get(mod_id), Some(ModFetchOutcome::Unavailable))));
}

#[tokio::test]
async fn a_refused_api_key_on_the_batch_mod_fetch_tells_the_user_to_update() {
    let server = FakeModio::start(api_key_refused_and_mods_gone);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);

    fetch_mod_outcomes_batch(&state, &PRIVATE_MODS).await;

    assert!(state.status().api_key_refused);
}

#[tokio::test]
async fn an_expired_token_while_listing_dependencies_is_not_reported_per_dependency() {
    capture_reports();
    let server = FakeModio::start(batch_omits_private_mods_with_expired_token);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);
    state.store_dependencies(100, PRIVATE_MODS.to_vec());

    let result = crate::modio_client::list_mod_dependencies_inner(&state, 100)
        .await
        .expect("dependency list should load");

    assert_eq!(sentry_reports(), Vec::<String>::new());
    let ids: Vec<u64> = result.mods.iter().map(|dependency| dependency.id).collect();
    assert_eq!(ids, PRIVATE_MODS);
}

/// A public mod whose read works with the api_key; the session token that
/// the rating read needs has expired.
fn public_mod_with_expired_token(request: &Received) -> (u16, Option<u32>, String) {
    if request.bearer {
        token_expired()
    } else {
        (200, None, r#"{"id":100,"name":"Public mod"}"#.to_string())
    }
}

#[tokio::test]
async fn an_expired_token_while_loading_the_user_rating_is_not_reported() {
    capture_reports();
    let server = FakeModio::start(public_mod_with_expired_token);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);

    let detail = crate::modio_client::get_mod_inner(&state, 100)
        .await
        .expect("mod detail should load without the rating");

    assert_eq!(sentry_reports(), Vec::<String>::new());
    assert_eq!(detail.id, 100);
    assert!(state.auth_status().session_expired);
}

#[tokio::test]
async fn an_expired_token_while_installing_dependencies_is_not_reported_per_mod() {
    capture_reports();
    let server = FakeModio::start(private_mods_with_expired_token);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);
    for mod_id in PRIVATE_MODS {
        state.store_latest_file_id(mod_id, 1);
    }

    let result = install_targets_internal(
        None,
        &state,
        Path::new("/nonexistent-game-dir"),
        PRIVATE_MODS.to_vec(),
        &mut Vec::new(),
        &mut HashMap::new(),
        false,
        true,
        None,
        None,
        None,
        Some(999),
    )
    .await;

    assert_eq!(sentry_reports(), Vec::<String>::new());
    let Err(error) = result else {
        panic!("the install should stop once the session is rejected");
    };
    assert!(error.credentials_rejected(), "{error}");
    assert_eq!(
        server.received().len(),
        2,
        "only the first dependency should be read: {:#?}",
        server.received()
    );
}

/// Public mod 100 depends on the private mods; their dependency reads need the
/// session token, which has expired.
fn public_mod_with_private_dependencies(request: &Received) -> (u16, Option<u32>, String) {
    if request.path == format!("/games/{GAME_ID}/mods/100/dependencies") && !request.bearer {
        let body = r#"{"data":[{"mod_id":101},{"mod_id":102},{"mod_id":103}],"result_total":3}"#;
        (200, None, body.to_string())
    } else {
        private_mods_with_expired_token(request)
    }
}

#[tokio::test]
async fn an_expired_token_while_resolving_install_order_stops_reading_dependencies() {
    capture_reports();
    let server = FakeModio::start(public_mod_with_private_dependencies);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);

    let order = collect_install_order(&state, 100).await.expect("install order");

    assert_eq!(sentry_reports(), Vec::<String>::new());
    assert!(order.contains(&100));
    assert_eq!(
        server.received().len(),
        3,
        "the root and the first private dependency only: {:#?}",
        server.received()
    );
}

#[tokio::test]
async fn an_expired_token_while_subscribing_is_a_rejected_credential() {
    capture_reports();
    let server = FakeModio::start(private_mods_with_expired_token);
    let state = ModioState::signed_in_against(server.base_url.clone(), TOKEN);

    let result = install_targets_internal(
        None,
        &state,
        Path::new("/nonexistent-game-dir"),
        vec![101],
        &mut Vec::new(),
        &mut HashMap::new(),
        true,
        true,
        None,
        None,
        None,
        Some(101),
    )
    .await;

    let Err(error) = result else {
        panic!("subscribing with an expired token should fail");
    };
    assert!(error.credentials_rejected(), "{error}");
    assert_eq!(sentry_reports(), Vec::<String>::new());
}

// --- Private mods while signed out (issue #68) --------------------------------

/// Private mods read without a session token: mod.io refuses the api_key read.
fn private_mods_signed_out(request: &Received) -> (u16, Option<u32>, String) {
    if request.path == format!("/games/{GAME_ID}/mods") {
        (200, None, EMPTY_LIST.to_string())
    } else {
        forbidden()
    }
}

#[tokio::test]
async fn private_mods_while_signed_out_are_not_reported_per_mod() {
    capture_reports();
    let server = FakeModio::start(private_mods_signed_out);
    let state = ModioState::signed_out_against(server.base_url.clone());

    let outcomes = fetch_mod_outcomes_batch(&state, &PRIVATE_MODS).await;
    let installed: HashSet<u64> = PRIVATE_MODS.into_iter().collect();
    let map = build_dependency_map(&state, &installed).await;

    assert_eq!(sentry_reports(), Vec::<String>::new());
    assert!(PRIVATE_MODS
        .iter()
        .all(|mod_id| matches!(outcomes.get(mod_id), Some(ModFetchOutcome::Failed(_)))));
    assert!(map.values().all(Vec::is_empty));
    assert_eq!(bearer_requests(&server), 0);
}

// --- Installed mod file deleted on mod.io (issue #75) ------------------------

/// mod.io `error_ref` for "The requested file id could not be found."
const MODFILE_NOT_FOUND_ERROR_REF: u32 = 14000;

/// Mod 100's installed file 7 was deleted or replaced by its author.
fn installed_file_deleted(_: &Received) -> (u16, Option<u32>, String) {
    let body = error_body(MODFILE_NOT_FOUND_ERROR_REF, "The requested file id could not be found.");
    (404, Some(MODFILE_NOT_FOUND_ERROR_REF), body)
}

#[tokio::test]
async fn a_deleted_installed_file_is_not_reported_when_resolving_its_version() {
    capture_reports();
    let server = FakeModio::start(installed_file_deleted);
    let state = ModioState::signed_out_against(server.base_url.clone());

    let label = resolve_installed_file_version(&state, 100, 7, None).await;

    assert_eq!(sentry_reports(), Vec::<String>::new());
    assert_eq!(label, "");
}

#[tokio::test]
async fn a_server_error_when_resolving_an_installed_version_is_still_reported() {
    capture_reports();
    let server = FakeModio::start(server_error);
    let state = ModioState::signed_out_against(server.base_url.clone());

    resolve_installed_file_version(&state, 100, 7, None).await;

    let caller_warnings: Vec<String> = sentry_reports()
        .into_iter()
        .filter(|report| report.starts_with("WARN: Did not resolve version"))
        .collect();
    assert_eq!(caller_warnings.len(), 1, "{caller_warnings:#?}");
}

#[tokio::test]
async fn a_deleted_installed_file_is_not_requested_again_on_the_next_refresh() {
    let server = FakeModio::start(installed_file_deleted);
    let state = ModioState::signed_out_against(server.base_url.clone());

    resolve_installed_file_version(&state, 100, 7, None).await;
    let label = resolve_installed_file_version(&state, 100, 7, None).await;

    assert_eq!(label, "");
    assert_eq!(server.received().len(), 1, "{:#?}", server.received());
}

/// Mod 100 still exists, but the version the user picked (file 7) was deleted.
fn picked_version_deleted(request: &Received) -> (u16, Option<u32>, String) {
    if request.path == format!("/games/{GAME_ID}/mods/100") {
        (200, None, r#"{"id":100,"name":"Some mod"}"#.to_string())
    } else {
        installed_file_deleted(request)
    }
}

async fn install_picked_version(state: &ModioState, file_id: u64) -> Result<InstallModResult, ModioError> {
    install_targets_internal(
        None,
        state,
        Path::new("/nonexistent-game-dir"),
        vec![100],
        &mut Vec::new(),
        &mut HashMap::new(),
        false,
        true,
        Some((100, file_id)),
        None,
        None,
        Some(100),
    )
    .await
}

#[tokio::test]
async fn installing_a_deleted_version_drops_the_stale_version_list() {
    let server = FakeModio::start(picked_version_deleted);
    let state = ModioState::signed_out_against(server.base_url.clone());
    state.store_latest_file_id(100, 8);
    state.store_mod_files(100, Vec::new());

    let _ = install_picked_version(&state, 7).await;

    assert!(state.cached_mod_files(100).is_none(), "the versions list should be fetched again");
}

#[tokio::test]
async fn installing_a_deleted_version_tells_the_user_and_is_not_reported() {
    capture_reports();
    let server = FakeModio::start(picked_version_deleted);
    let state = ModioState::signed_out_against(server.base_url.clone());
    state.store_latest_file_id(100, 8);

    let result = install_picked_version(&state, 7).await;

    assert_eq!(sentry_reports(), Vec::<String>::new());
    let Err(error) = result else {
        panic!("installing a deleted version should fail");
    };
    assert_eq!(
        error.message,
        "This version of Some mod is no longer on mod.io. Its author deleted or replaced it. Pick another version, or install the latest one."
    );
    assert_eq!(error.log_level(log::Level::Error), log::Level::Info);
    assert!(error.is_modfile_deleted(), "a restored profile falls back on this");
}
