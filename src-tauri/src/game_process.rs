use serde::Serialize;
use sysinfo::{ProcessesToUpdate, System};

const GAME_PROCESS_BASENAMES: &[&str] = &["zeepkist", "zeepkist.exe"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRunningStatus {
    pub running: bool,
    pub message: Option<String>,
}

fn process_name_is_game(name: &str) -> bool {
    let basename = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name);
    let lower = basename.to_ascii_lowercase();
    GAME_PROCESS_BASENAMES
        .iter()
        .any(|candidate| lower == *candidate)
}

pub fn is_zeepkist_running() -> bool {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .processes()
        .values()
        .any(|process| process_name_is_game(&process.name().to_string_lossy()))
}

pub fn ensure_game_not_running() -> Result<(), String> {
    if is_zeepkist_running() {
        return Err(
            "Zeepkist is running. Close the game before installing, updating, or removing mods."
                .into(),
        );
    }
    Ok(())
}

fn build_status() -> GameRunningStatus {
    let running = is_zeepkist_running();
    GameRunningStatus {
        running,
        message: if running {
            Some(
                "Zeepkist is running. Close the game before installing, updating, or removing mods."
                    .into(),
            )
        } else {
            None
        },
    }
}

#[tauri::command]
pub fn game_running_status() -> GameRunningStatus {
    build_status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_process_names() {
        assert!(process_name_is_game("zeepkist"));
        assert!(process_name_is_game("Zeepkist.exe"));
        assert!(process_name_is_game("C:\\Games\\Zeepkist\\zeepkist.exe"));
        assert!(process_name_is_game("/path/to/Zeepkist/zeepkist.exe"));
        assert!(!process_name_is_game("wine64-preloader"));
        assert!(!process_name_is_game("not-zeepkist.exe"));
    }
}
