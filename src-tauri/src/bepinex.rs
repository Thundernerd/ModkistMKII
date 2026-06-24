use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use pelite::pe32::{Pe as Pe32, PeFile as PeFile32};
use pelite::pe64::{Pe as Pe64, PeFile as PeFile64};
use pelite::resources::version_info::VersionInfo;
use serde::Serialize;
use tauri::AppHandle;

use crate::game_path::game_directory;
use crate::zip_extract::extract_zip;

const REQUIRED_VERSION: &str = "5.4.20";
const DOWNLOAD_URL: &str =
    "https://github.com/BepInEx/BepInEx/releases/download/v5.4.20/BepInEx_x64_5.4.20.0.zip";

const WINHTTP_DLL: &str = "winhttp.dll";
const DOORSTOP_CONFIG: &str = "doorstop_config.ini";
const BEPINEX_DIR: &str = "BepInEx";
const CHANGELOG_TXT: &str = "changelog.txt";
const PRELOADER_DLL: &str = "BepInEx/core/BepInEx.Preloader.dll";
const BEPINEX_DLL: &str = "BepInEx/core/BepInEx.dll";
const BEPINEX_PRELOADER_DLL: &str = "BepInEx/core/BepInEx.Preloader.dll";
const VERSION_PROBE_DLLS: [&str; 2] = [BEPINEX_DLL, BEPINEX_PRELOADER_DLL];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BepInExState {
    Missing,
    Installed,
    WrongVersion,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BepInExStatus {
    pub state: String,
    pub found_version: Option<String>,
    pub message: Option<String>,
    pub can_continue: bool,
}

impl BepInExStatus {
    fn from_detection(state: BepInExState, found_version: Option<String>) -> Self {
        match state {
            BepInExState::Missing => Self {
                state: "missing".into(),
                found_version,
                message: Some(format!(
                    "BepInEx {REQUIRED_VERSION} (x64) was not found in your game directory."
                )),
                can_continue: false,
            },
            BepInExState::Installed => Self {
                state: "installed".into(),
                found_version,
                message: None,
                can_continue: true,
            },
            BepInExState::WrongVersion => Self {
                state: "wrongVersion".into(),
                found_version: found_version.clone(),
                message: Some(format!(
                    "Found BepInEx {}. Modkist expects {REQUIRED_VERSION}. You can continue, but some mods may not work.",
                    found_version.unwrap_or_else(|| "an unknown version".into())
                )),
                can_continue: true,
            },
        }
    }
}

pub(crate) fn has_bepinex_structure(game_dir: &Path) -> bool {
    game_dir.join(WINHTTP_DLL).is_file()
        && game_dir.join(DOORSTOP_CONFIG).is_file()
        && game_dir.join(PRELOADER_DLL).is_file()
}

fn read_pe_version(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    read_pe_version_bytes(&bytes).ok_or_else(|| {
        format!(
            "Could not read version info from {}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("PE file")
        )
    })
}

fn read_pe_version_bytes(bytes: &[u8]) -> Option<String> {
    if let Ok(pe) = PeFile64::from_bytes(bytes) {
        if let Some(version) = read_version_from_pe64(pe) {
            return Some(version);
        }
    }

    if let Ok(pe) = PeFile32::from_bytes(bytes) {
        if let Some(version) = read_version_from_pe32(pe) {
            return Some(version);
        }
    }

    None
}

fn read_version_from_pe64(pe: PeFile64<'_>) -> Option<String> {
    let resources = pe.resources().ok()?;
    let version_info = resources.version_info().ok()?;
    read_version_strings(version_info).or_else(|| read_fixed_version(version_info))
}

fn read_version_from_pe32(pe: PeFile32<'_>) -> Option<String> {
    let resources = pe.resources().ok()?;
    let version_info = resources.version_info().ok()?;
    read_version_strings(version_info).or_else(|| read_fixed_version(version_info))
}

fn read_fixed_version(version_info: VersionInfo<'_>) -> Option<String> {
    let fixed = version_info.fixed()?;
    Some(format!(
        "{}.{}.{}.{}",
        fixed.dwFileVersion.Major,
        fixed.dwFileVersion.Minor,
        fixed.dwFileVersion.Patch,
        fixed.dwFileVersion.Build
    ))
}

fn read_version_strings(version_info: VersionInfo<'_>) -> Option<String> {
    let langs = version_info.translation();
    let lang = langs.first().copied()?;

    version_info
        .value(lang, "FileVersion")
        .or_else(|| version_info.value(lang, "ProductVersion"))
}

fn version_matches_required(version: &str) -> bool {
    version.trim().starts_with(REQUIRED_VERSION)
}

fn detect_bepinex(game_dir: &Path) -> (BepInExState, Option<String>) {
    if !has_bepinex_structure(game_dir) {
        return (BepInExState::Missing, None);
    }

    for relative_path in VERSION_PROBE_DLLS {
        let dll_path = game_dir.join(relative_path);
        if !dll_path.is_file() {
            continue;
        }

        match read_pe_version(&dll_path) {
            Ok(version) if version_matches_required(&version) => {
                return (BepInExState::Installed, Some(version));
            }
            Ok(version) => {
                return (BepInExState::WrongVersion, Some(version));
            }
            Err(_) => continue,
        }
    }

    (BepInExState::WrongVersion, None)
}

async fn download_archive(destination: &Path) -> Result<(), String> {
    let response = reqwest::get(DOWNLOAD_URL)
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed with status {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    let mut file = File::create(destination)
        .map_err(|e| format!("Could not create temp file: {e}"))?;
    file.write_all(&bytes)
        .map_err(|e| format!("Could not write temp file: {e}"))?;

    Ok(())
}

fn status_for_game_dir(game_dir: &Path) -> BepInExStatus {
    let (state, found_version) = detect_bepinex(game_dir);
    BepInExStatus::from_detection(state, found_version)
}

#[tauri::command]
pub fn bepinex_status(app: AppHandle) -> Result<BepInExStatus, String> {
    let game_dir = game_directory(&app)?;
    Ok(status_for_game_dir(&game_dir))
}

#[tauri::command]
pub async fn install_bepinex(app: AppHandle) -> Result<BepInExStatus, String> {
    let game_dir = game_directory(&app)?;
    let (state, _) = detect_bepinex(&game_dir);

    if state == BepInExState::Installed {
        return Ok(status_for_game_dir(&game_dir));
    }

    if state == BepInExState::WrongVersion {
        return Err(
            "A different BepInEx version is already installed. Use reinstall from Settings or remove it manually."
                .into(),
        );
    }

    perform_install(&game_dir).await
}

async fn perform_install(game_dir: &Path) -> Result<BepInExStatus, String> {
    let temp_dir = std::env::temp_dir().join("modkist-bepinex");
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Could not create temp directory: {e}"))?;
    let archive_path = temp_dir.join("BepInEx_x64_5.4.20.0.zip");

    download_archive(&archive_path).await?;
    extract_zip(&archive_path, game_dir)?;
    let _ = fs::remove_file(&archive_path);

    let status = status_for_game_dir(game_dir);
    if status.state != "installed" {
        return Err(
            "BepInEx was extracted but verification failed. Check your game directory and try again."
                .into(),
        );
    }

    Ok(status)
}

fn remove_bepinex_installation(game_dir: &Path) -> Result<(), String> {
    let bepinex_dir = game_dir.join(BEPINEX_DIR);
    if bepinex_dir.exists() {
        fs::remove_dir_all(&bepinex_dir).map_err(|e| {
            format!(
                "Could not remove BepInEx directory {}: {e}",
                bepinex_dir.display()
            )
        })?;
    }

    for file_name in [WINHTTP_DLL, DOORSTOP_CONFIG, CHANGELOG_TXT] {
        let path = game_dir.join(file_name);
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| {
                format!("Could not remove {}: {e}", path.display())
            })?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn reinstall_bepinex(app: AppHandle) -> Result<BepInExStatus, String> {
    let game_dir = game_directory(&app)?;
    remove_bepinex_installation(&game_dir)?;
    perform_install(&game_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bepinex_version_from_pe32_dll() {
        let dll_path = std::path::Path::new("/tmp/bepinex-test/BepInEx/core/BepInEx.dll");
        if !dll_path.is_file() {
            return;
        }

        let version = read_pe_version(dll_path).expect("version should be readable");
        assert!(
            version_matches_required(&version),
            "expected {REQUIRED_VERSION}, got {version}"
        );
    }
}
