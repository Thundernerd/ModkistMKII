use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const GAME_STORE_PATH: &str = "zeepkist-game.json";
const GAME_DIRECTORY_KEY: &str = "gameDirectoryPath";
const GAME_EXECUTABLE: &str = "zeepkist.exe";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePathStatus {
    pub configured: bool,
    pub valid: bool,
    pub path: Option<String>,
    pub message: Option<String>,
}

fn validate_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("This directory does not exist.".into());
    }

    if !path.is_dir() {
        return Err("The path must be a directory.".into());
    }

    let executable = path.join(GAME_EXECUTABLE);
    if !executable.is_file() {
        return Err(format!(
            "Could not find {GAME_EXECUTABLE} in this directory."
        ));
    }

    Ok(())
}

pub fn game_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let path = read_stored_path(app)?
        .ok_or_else(|| "Game directory is not configured.".to_string())?;
    let path_buf = PathBuf::from(&path);
    validate_directory(&path_buf)?;
    Ok(path_buf)
}

fn read_stored_path(app: &AppHandle) -> Result<Option<String>, String> {
    let store = app.store(GAME_STORE_PATH).map_err(|e| e.to_string())?;
    Ok(store
        .get(GAME_DIRECTORY_KEY)
        .and_then(|value| value.as_str().map(str::to_string))
        .filter(|value| !value.is_empty()))
}

fn build_status(app: &AppHandle) -> GamePathStatus {
    match read_stored_path(app) {
        Ok(Some(path)) => {
            let path_buf = PathBuf::from(&path);
            match validate_directory(&path_buf) {
                Ok(()) => GamePathStatus {
                    configured: true,
                    valid: true,
                    path: Some(path),
                    message: None,
                },
                Err(message) => GamePathStatus {
                    configured: true,
                    valid: false,
                    path: Some(path),
                    message: Some(message),
                },
            }
        }
        Ok(None) => GamePathStatus {
            configured: false,
            valid: false,
            path: None,
            message: Some("Select your Zeepkist game directory.".into()),
        },
        Err(message) => GamePathStatus {
            configured: false,
            valid: false,
            path: None,
            message: Some(message),
        },
    }
}

#[tauri::command]
pub fn game_path_status(app: AppHandle) -> GamePathStatus {
    build_status(&app)
}

#[tauri::command]
pub fn set_game_path(app: AppHandle, path: String) -> Result<GamePathStatus, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Enter your Zeepkist game directory.".into());
    }

    let path_buf = PathBuf::from(trimmed);
    validate_directory(&path_buf)?;

    let store = app.store(GAME_STORE_PATH).map_err(|e| e.to_string())?;
    store.set(GAME_DIRECTORY_KEY, serde_json::json!(trimmed));
    store.save().map_err(|e| e.to_string())?;

    log::info!("Game directory set to {}", trimmed);
    Ok(build_status(&app))
}
