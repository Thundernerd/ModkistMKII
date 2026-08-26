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
                .map_err(|error| error.to_string())?;
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
            log::warn!("App update check failed: {error}");
            Err(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
