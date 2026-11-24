use std::time::Duration;

use reqwest::StatusCode;
use serde::Serialize;
use tauri::utils::{config::BundleType, platform::bundle_type};
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

/// Whether installing an update asks for the user's password. The updater
/// installs a deb or rpm through the system package manager, as root.
fn install_needs_password(bundle: Option<BundleType>) -> bool {
    matches!(bundle, Some(BundleType::Deb | BundleType::Rpm))
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
        // A body that is not JSON is a broken manifest; anything else is the request.
        Error::Reqwest(request) if !request.is_decode() => {
            log::info!("App update check failed: {error}")
        }
        _ => log::warn!("App update check failed: {error}"),
    }
}

/// Whether the step that asks for the password and installs the package failed.
/// The updater returns the same errors for a cancelled prompt as for a package
/// the system refused.
fn is_package_install_failure(error: &tauri_plugin_updater::Error, needs_password: bool) -> bool {
    use tauri_plugin_updater::Error;

    match error {
        Error::AuthenticationFailed | Error::DebInstallFailed | Error::PackageInstallFailed => true,
        // After a cancelled prompt the updater falls back to sudo, which may not exist.
        Error::Io(_) => needs_password,
        _ => false,
    }
}

/// Log a failed update download or install. This runs only when an update is
/// available, so it is rare enough to report: a missing installer or a bad
/// signature locks every user out until a fixed release. The exception is a
/// failed package install, which stays in the log file because it is mostly
/// users cancelling the password prompt.
fn log_install_failure(error: &tauri_plugin_updater::Error, needs_password: bool) {
    if is_package_install_failure(error, needs_password) {
        log::info!("App update install failed: {error}");
    } else {
        log::warn!("App update install failed: {error}");
    }
}

/// Check for an update and install it. When installing needs the user's password
/// and `allow_password_prompt` is false, nothing is downloaded and the status is
/// `needsPassword`, so that the prompt only follows something the user clicked.
///
/// An update that could not be downloaded or installed is the status
/// `installFailed`; `Err` means the check itself failed. The update gate keeps
/// the app closed for the first and opens it for the second, so that being
/// offline or GitHub being down does not lock users out.
#[tauri::command]
pub async fn check_and_install_app_update(
    app: AppHandle,
    allow_password_prompt: bool,
) -> Result<AppUpdateCheckResult, String> {
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
            let needs_password = install_needs_password(bundle_type());
            if needs_password && !allow_password_prompt {
                log::info!("Modkist {version} is available; installing it needs the password");
                return Ok(AppUpdateCheckResult {
                    status: "needsPassword".into(),
                    version: Some(version),
                });
            }

            log::info!("Downloading Modkist {version}");
            let _ = app.emit(
                APP_UPDATE_AVAILABLE_EVENT,
                AppUpdateAvailablePayload {
                    version: version.clone(),
                },
            );

            let progress_app = app.clone();
            let mut downloaded: u64 = 0;
            let installed = update
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
                .await;
            if let Err(error) = installed {
                log_install_failure(&error, needs_password);
                return Ok(AppUpdateCheckResult {
                    status: "installFailed".into(),
                    version: Some(version),
                });
            }
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

        log_check_failure(
            &UpdaterError::ReleaseNotFound,
            "http://127.0.0.1:1/latest.json",
        )
        .await;

        assert_eq!(sentry_reports(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_missing_update_manifest_is_reported_to_sentry() {
        let (_server, endpoint) = manifest_endpoint(404);
        capture_reports();

        log_check_failure(&UpdaterError::ReleaseNotFound, &endpoint).await;

        let reports = sentry_reports();
        assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
        assert!(
            reports[0].starts_with("WARN: App update check failed"),
            "{}",
            reports[0]
        );
        assert!(
            reports[0].contains("404"),
            "report should name the status: {}",
            reports[0]
        );
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
    async fn a_manifest_that_is_not_json_is_reported_to_sentry() {
        // What the plugin returns for a 200 with an HTML page or a corrupt upload.
        let response = tauri::http::Response::builder()
            .status(200)
            .body(b"<html>Not a manifest</html>".to_vec())
            .unwrap();
        let not_json = updater_reqwest::Response::from(response)
            .json::<serde_json::Value>()
            .await
            .expect_err("HTML is not JSON");
        capture_reports();

        log_check_failure(&UpdaterError::Reqwest(not_json), UNUSED_ENDPOINT).await;

        let reports = sentry_reports();
        assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
        assert!(
            reports[0].starts_with("WARN: App update check failed"),
            "{}",
            reports[0]
        );
    }

    #[tokio::test]
    async fn a_manifest_without_this_platform_is_still_reported_to_sentry() {
        capture_reports();

        log_check_failure(
            &UpdaterError::TargetNotFound("windows-x86_64".into()),
            UNUSED_ENDPOINT,
        )
        .await;

        let reports = sentry_reports();
        assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
        assert!(
            reports[0].starts_with("WARN: App update check failed"),
            "{}",
            reports[0]
        );
        assert!(
            reports[0].contains("windows-x86_64"),
            "report should name the platform: {}",
            reports[0]
        );
    }

    #[test]
    fn a_missing_installer_is_reported_to_sentry() {
        // The manifest points at an installer asset that is not there.
        for needs_password in [false, true] {
            capture_reports();

            log_install_failure(
                &UpdaterError::Network("Download request failed with status: 404 Not Found".into()),
                needs_password,
            );

            let reports = sentry_reports();
            assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
            assert!(
                reports[0].contains("404"),
                "report should name the status: {}",
                reports[0]
            );
        }
    }

    #[test]
    fn a_cancelled_password_prompt_is_not_reported_to_sentry() {
        capture_reports();

        for error in [
            UpdaterError::AuthenticationFailed,
            UpdaterError::PackageInstallFailed,
            UpdaterError::Io(std::io::ErrorKind::NotFound.into()),
        ] {
            log_install_failure(&error, true);
        }

        assert_eq!(sentry_reports(), Vec::<String>::new());
    }

    #[test]
    fn a_failed_write_without_a_password_prompt_is_reported_to_sentry() {
        capture_reports();

        // Replacing the AppImage or the macOS app bundle.
        log_install_failure(
            &UpdaterError::Io(std::io::ErrorKind::PermissionDenied.into()),
            false,
        );

        assert_eq!(sentry_reports().len(), 1);
    }

    #[test]
    fn an_installer_with_a_bad_signature_is_reported_to_sentry() {
        capture_reports();

        log_install_failure(&UpdaterError::SignatureUtf8("not-base64".into()), true);

        let reports = sentry_reports();
        assert_eq!(reports.len(), 1, "expected one report, got {reports:#?}");
        assert!(
            reports[0].starts_with("WARN: App update install failed"),
            "{}",
            reports[0]
        );
    }

    #[test]
    fn only_deb_and_rpm_installs_need_the_password() {
        assert!(install_needs_password(Some(BundleType::Deb)));
        assert!(install_needs_password(Some(BundleType::Rpm)));

        for bundle in [
            BundleType::AppImage,
            BundleType::Msi,
            BundleType::Nsis,
            BundleType::App,
            BundleType::Dmg,
        ] {
            assert!(!install_needs_password(Some(bundle.clone())), "{bundle:?}");
        }
        assert!(!install_needs_password(None));
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
