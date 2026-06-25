use std::path::Path;
#[cfg(any(windows, target_os = "linux"))]
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tauri::AppHandle;

use crate::game_path::game_directory;
use crate::game_process::is_zeepkist_running;

#[cfg(any(windows, target_os = "linux"))]
const STEAM_APP_ID: &str = "1440670";
const GAME_EXECUTABLE: &str = "zeepkist.exe";

fn spawn_detached(command: &mut Command) -> Result<(), String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not start the game: {error}"))
}

#[cfg(windows)]
fn launch_direct_exe(game_dir: &Path) -> Result<(), String> {
    let executable = game_dir.join(GAME_EXECUTABLE);
    if !executable.is_file() {
        return Err(format!(
            "Could not find {GAME_EXECUTABLE} in {}",
            game_dir.display()
        ));
    }

    Command::new(&executable)
        .current_dir(game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not start the game: {error}"))
}

#[cfg(any(windows, target_os = "linux"))]
fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(any(windows, target_os = "linux"))]
fn find_steam_executable() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(program_files_x86).join("Steam/steam.exe"));
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            candidates.push(PathBuf::from(program_files).join("Steam/steam.exe"));
        }

        for candidate in candidates {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }

        return Err("Could not find Steam. Install Steam or launch Zeepkist manually.".into());
    }

    #[cfg(target_os = "linux")]
    {
        if command_exists("steam") {
            return Ok(PathBuf::from("steam"));
        }

        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let candidates = [
                home.join(".local/share/Steam/ubuntu12_32/steam"),
                home.join(".steam/steam/ubuntu12_32/steam"),
                home.join(".steam/debian-installation/ubuntu12_32/steam"),
            ];
            for candidate in candidates {
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }

        Err("Could not find Steam. Install Steam or launch Zeepkist manually.".into())
    }
}

#[cfg(any(windows, target_os = "linux"))]
fn launch_via_steam() -> Result<(), String> {
    let steam = find_steam_executable()?;
    let mut command = Command::new(&steam);
    command.args(["-applaunch", STEAM_APP_ID]);
    spawn_detached(&mut command)
}

#[cfg(unix)]
fn launch_via_wine(game_dir: &Path) -> Result<(), String> {
    let info = crate::wine_prefix::wine_launch_info(game_dir).ok_or_else(|| {
        "Could not find a Wine prefix for your game directory. Launch Zeepkist from Steam or CrossOver."
            .to_string()
    })?;

    let executable = game_dir.join(GAME_EXECUTABLE);
    if !executable.is_file() {
        return Err(format!(
            "Could not find {GAME_EXECUTABLE} in {}",
            game_dir.display()
        ));
    }

    #[cfg(target_os = "macos")]
    if let Some(bottle) = info
        .bottle_name
        .as_deref()
        .filter(|name| !name.starts_with("Steam app"))
    {
        let mut command = Command::new(&info.wine);
        command
            .args(["--bottle", bottle, "--cx-app", GAME_EXECUTABLE])
            .current_dir(game_dir);
        return spawn_detached(&mut command);
    }

    let mut command = Command::new(&info.wine);
    command
        .env("WINEPREFIX", &info.prefix)
        .arg(&executable)
        .current_dir(game_dir);
    spawn_detached(&mut command)
}

fn launch_game_at(game_dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        if launch_via_steam().is_ok() {
            log::info!("Launched Zeepkist via Steam (app {STEAM_APP_ID})");
            return Ok(());
        }

        log::warn!("Steam launch failed, falling back to direct executable");
        launch_direct_exe(game_dir)?;
        log::info!("Launched Zeepkist directly from {}", game_dir.display());
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        if launch_via_steam().is_ok() {
            log::info!("Launched Zeepkist via Steam (app {STEAM_APP_ID})");
            return Ok(());
        }

        log::warn!("Steam launch failed, falling back to Wine");
        launch_via_wine(game_dir)?;
        log::info!("Launched Zeepkist via Wine from {}", game_dir.display());
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        launch_via_wine(game_dir)?;
        log::info!("Launched Zeepkist via CrossOver/Wine from {}", game_dir.display());
        return Ok(());
    }
}

#[tauri::command]
pub fn launch_game(app: AppHandle) -> Result<(), String> {
    if is_zeepkist_running() {
        return Err("Zeepkist is already running.".into());
    }

    let game_dir = game_directory(&app)?;
    launch_game_at(&game_dir)
}

#[cfg(test)]
mod tests {
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn steam_app_id_is_zeepkist() {
        assert_eq!(super::STEAM_APP_ID, "1440670");
    }
}
