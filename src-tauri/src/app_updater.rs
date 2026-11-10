use std::time::Duration;

use reqwest::StatusCode;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

const UPDATER_STABLE_MANIFEST_URL: &str =
    "https://github.com/Thundernerd/ModkistMKII/releases/download/updater/latest.json";
const UPDATER_PRERELEASE_MANIFEST_URL: &str =
    "https://github.com/Thundernerd/ModkistMKII/releases/download/updater/latest-prerelease.json";

const APP_UPDATE_AVAILABLE_EVENT: &str = "app-update://available";
const APP_UPDATE_PROGRESS_EVENT: &str = "app-update://progress";

pub fn updater_endpoint_for_version(version: &str) -> &'static str {
    if version.contains('-') {
        UPDATER_PRERELEASE_MANIFEST_URL
    } else {
        UPDATER_STABLE_MANIFEST_URL
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateCheckResult {
    pub status: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateAvailablePayload {
    version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateProgressPayload {
    downloaded: u64,
    total: Option<u64>,
}

fn skipped_result() -> AppUpdateCheckResult {
    AppUpdateCheckResult {
        status: "skipped".into(),
        version: None,
    }
}

/// Whether an updater error is a network outcome that the next launch retries:
/// offline, captive portal, a GitHub hiccup, or the manifest being replaced
/// mid-release. Anything else means the published manifest or update is broken.
fn is_transient(error: &tauri_plugin_updater::Error) -> bool {
    use tauri_plugin_updater::Error;

    matches!(
        error,
        Error::ReleaseNotFound | Error::Reqwest(_) | Error::Network(_)
    )
}

/// Log a failed updater step. Transient failures stay in the log file; the
/// rest are reported to Sentry.
fn log_updater_failure(step: &str, error: &tauri_plugin_updater::Error) {
    if is_transient(error) {
        log::info!("App update {step} failed: {error}");
    } else {
        log::warn!("App update {step} failed: {error}");
    }
}

/// How long the follow-up request for a missing manifest's status may take.
const MANIFEST_STATUS_TIMEOUT: Duration = Duration::from_secs(10);

/// The status the manifest endpoint answers with now, or `None` without a response.
/// `ReleaseNotFound` drops the status the updater saw, so ask once more.
async fn manifest_status(endpoint: &str) -> Option<StatusCode> {
    let client = reqwest::Client::builder()
        .timeout(MANIFEST_STATUS_TIMEOUT)
        .build()
        .ok()?;
    let response = client.get(endpoint).send().await.ok()?;
    Some(response.status())
}

/// Whether the manifest is missing or refused for everyone, as opposed to
/// GitHub, a proxy or the user's network having a bad moment.
fn is_missing_manifest(status: StatusCode) -> bool {
    status.is_client_error()
        && status != StatusCode::TOO_MANY_REQUESTS
        && status != StatusCode::REQUEST_TIMEOUT
}

/// Log a failed update check. A missing or broken manifest is reported to
/// Sentry; a network failure stays in the log file, since the next launch
/// checks again.
async fn log_check_failure(error: &tauri_plugin_updater::Error, endpoint: &str) {
    use tauri_plugin_updater::Error;

    match error {
        Error::ReleaseNotFound => match manifest_status(endpoint).await {
            Some(status) if is_missing_manifest(status) => {
                log::warn!("App update check failed: manifest {endpoint} returned {status}")
            }
            Some(status) => {
                log::info!("App update check failed: manifest {endpoint} returned {status}")
            }
            None => log::info!("App update check failed: manifest {endpoint} did not respond"),
        },
        Error::Reqwest(_) => log::info!("App update check failed: {error}"),
        _ => log::warn!("App update check failed: {error}"),
    }
}

fn log_install_failure(error: &tauri_plugin_updater::Error) {
    log_updater_failure("install", error);
}

#[tauri::command]
pub async fn check_and_install_app_update(app: AppHandle) -> Result<AppUpdateCheckResult, String> {
    if cfg!(debug_assertions) {
        return Ok(skipped_result());
    }

    let endpoint = updater_endpoint_for_version(env!("CARGO_PKG_VERSION"));
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint
            .parse()
            .map_err(|error: url::ParseError| error.to_string())?])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;

    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            log::info!("Downloading Modkist {version}");
            let _ = app.emit(
                APP_UPDATE_AVAILABLE_EVENT,
                AppUpdateAvailablePayload {
                    version: version.clone(),
                },
            );

            let progress_app = app.clone();
            let mut downloaded: u64 = 0;
            update
                .download_and_install(
                    |chunk_length, content_length| {
                        downloaded += chunk_length as u64;
                        let _ = progress_app.emit(
                            APP_UPDATE_PROGRESS_EVENT,
                            AppUpdateProgressPayload {
                                downloaded,
                                total: content_length,
                            },
                        );
                    },
                    || {},
                )
                .await
                .map_err(|error| {
                    log_install_failure(&error);
                    error.to_string()
                })?;
            log::info!("Installed Modkist {version}; waiting for restart");
            Ok(AppUpdateCheckResult {
                status: "installed".into(),
                version: Some(version),
            })
        }
        Ok(None) => Ok(AppUpdateCheckResult {
            status: "upToDate".into(),
            version: None,
        }),
        Err(error) => {
            log_check_failure(&error, endpoint).await;
            Err(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::report_capture::{capture_reports, sentry_reports};
    use crate::modio_api::auth_tests::FakeModio;
    use tauri_plugin_updater::Error as UpdaterError;

    /// For failures that are classified without asking the endpoint again.
    const UNUSED_ENDPOINT: &str = "http://127.0.0.1:1/unused.json";

    /// A manifest endpoint on a local server that answers every request with `status`.
    fn manifest_endpoint(status: u16) -> (FakeModio, String) {
        let server = FakeModio::start(move |_| (status, None, String::new()));
        let endpoint = format!("{}/latest.json", server.base_url);
        (server, endpoint)
    }

    #[tokio::test]
    async fn an_update_endpoint_with_a_server_error_is_not_reported_to_sentry() {
        // GitHub hiccup, or the manifest being replaced mid-release.
        let (_server, endpoint) = manifest_endpoint(503);
        capture_reports();

        log_check_failure(&UpdaterError::ReleaseNotFound, &endpoint).await;

        assert_eq!(sentry_reports(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn an_unreachable_update_endpoint_is_not_reported_to_sentry() {
        // Offline: nothing listens on port 1.
        capture_reports();

        log_check_failure(&UpdaterError::ReleaseNotFound, "http://127.0.0.1:1/latest.json").await;

        assert_eq!(sentry_reports(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_missing_update_manifest_is_reported_to_sentry() {
        let (_server, endpoint) = manifest_endpoint(404);
        capture_reports();

        log_check_failure(&UpdaterError::ReleaseNotFound, &endpoint).await;

        let reports = sentry_reports();
        assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
        assert!(reports[0].starts_with("WARN: App update check failed"), "{}", reports[0]);
        assert!(reports[0].contains("404"), "report should name the status: {}", reports[0]);
    }

    /// A `reqwest::Error` of the major tauri-plugin-updater uses. Building a real
    /// client needs a rustls crypto provider, so convert a response instead.
    fn failed_request() -> updater_reqwest::Error {
        let response = tauri::http::Response::builder()
            .status(503)
            .body(Vec::<u8>::new())
            .unwrap();
        updater_reqwest::Response::from(response)
            .error_for_status()
            .expect_err("a 503 is an error status")
    }

    #[tokio::test]
    async fn a_failed_update_request_is_not_reported_to_sentry() {
        capture_reports();

        // Offline, DNS, TLS or a cut-off body while fetching the manifest.
        log_check_failure(&UpdaterError::Reqwest(failed_request()), UNUSED_ENDPOINT).await;

        assert_eq!(sentry_reports(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_manifest_without_this_platform_is_still_reported_to_sentry() {
        capture_reports();

        log_check_failure(&UpdaterError::TargetNotFound("windows-x86_64".into()), UNUSED_ENDPOINT).await;

        let reports = sentry_reports();
        assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
        assert!(reports[0].starts_with("WARN: App update check failed"), "{}", reports[0]);
        assert!(reports[0].contains("windows-x86_64"), "report should name the platform: {}", reports[0]);
    }

    #[test]
    fn a_failed_installer_download_is_not_reported_to_sentry() {
        capture_reports();

        log_install_failure(&UpdaterError::Network(
            "Download request failed with status: 502 Bad Gateway".into(),
        ));

        assert_eq!(sentry_reports(), Vec::<String>::new());
    }

    #[test]
    fn an_installer_with_a_bad_signature_is_reported_to_sentry() {
        capture_reports();

        log_install_failure(&UpdaterError::SignatureUtf8("not-base64".into()));

        let reports = sentry_reports();
        assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
        assert!(reports[0].starts_with("WARN: App update install failed"), "{}", reports[0]);
    }

    #[test]
    fn stable_versions_use_stable_manifest() {
        assert_eq!(
            updater_endpoint_for_version("1.0.0"),
            UPDATER_STABLE_MANIFEST_URL
        );
    }

    #[test]
    fn prerelease_versions_use_prerelease_manifest() {
        assert_eq!(
            updater_endpoint_for_version("1.0.0-rc.9"),
            UPDATER_PRERELEASE_MANIFEST_URL
        );
        assert_eq!(
            updater_endpoint_for_version("1.2.3-beta.2"),
            UPDATER_PRERELEASE_MANIFEST_URL
        );
        assert_eq!(
            updater_endpoint_for_version("1.2.3-alpha.1"),
            UPDATER_PRERELEASE_MANIFEST_URL
        );
    }

    #[test]
    fn available_payload_serializes_version() {
        let json = serde_json::to_value(AppUpdateAvailablePayload {
            version: "1.2.3".into(),
        })
        .unwrap();
        assert_eq!(json["version"], "1.2.3");
    }

    #[test]
    fn progress_payload_serializes_optional_total() {
        let json = serde_json::to_value(AppUpdateProgressPayload {
            downloaded: 1024,
            total: Some(2048),
        })
        .unwrap();
        assert_eq!(json["downloaded"], 1024);
        assert_eq!(json["total"], 2048);

        let json = serde_json::to_value(AppUpdateProgressPayload {
            downloaded: 512,
            total: None,
        })
        .unwrap();
        assert_eq!(json["downloaded"], 512);
        assert!(json["total"].is_null());
    }
}
