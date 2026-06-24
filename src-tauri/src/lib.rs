mod auth;
mod modio_client;

use auth::{auth_status, logout, request_email_code, verify_email_code};
use modio_client::{list_mods, modio_status, ModioState};
use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();
    dotenvy::from_filename("../.env").ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = ModioState::from_env();
            if let Err(error) = state.restore_from_store(app.handle()) {
                eprintln!("Failed to restore mod.io session: {error}");
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            modio_status,
            list_mods,
            request_email_code,
            verify_email_code,
            auth_status,
            logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
