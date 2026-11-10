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

fn log_check_failure(error: &tauri_plugin_updater::Error) {
    log_updater_failure("check", error);
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
            log_check_failure(&error);
            Err(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modio_api::auth_tests::{capture_reports, sentry_reports};
    use tauri_plugin_updater::Error as UpdaterError;

    #[test]
    fn an_unreachable_update_endpoint_is_not_reported_to_sentry() {
        capture_reports();

        // A non-2xx response from every endpoint (GitHub hiccup, captive portal,
        // proxy, manifest briefly replaced during a release).
        log_check_failure(&UpdaterError::ReleaseNotFound);

        assert_eq!(sentry_reports(), Vec::<String>::new());
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

    #[test]
    fn a_failed_update_request_is_not_reported_to_sentry() {
        capture_reports();

        // Offline, DNS, TLS or a cut-off body while fetching the manifest.
        log_check_failure(&UpdaterError::Reqwest(failed_request()));

        assert_eq!(sentry_reports(), Vec::<String>::new());
    }

    #[test]
    fn a_manifest_without_this_platform_is_still_reported_to_sentry() {
        capture_reports();

        log_check_failure(&UpdaterError::TargetNotFound("windows-x86_64".into()));

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
