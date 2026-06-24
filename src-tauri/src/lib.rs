mod auth;
mod game_path;
mod modio_client;

use auth::{auth_status, logout, request_email_code, verify_email_code};
use game_path::{game_path_status, set_game_path};
use modio_client::{
    get_mod, get_user_profile, list_mod_dependencies, list_mods, list_user_mods, modio_status,
    ModioState,
};
use tauri::webview::PageLoadEvent;
use tauri::Manager;

const RELOAD_IF_BLANK_JS: &str = r#"window.setTimeout(function () {
  var root = document.getElementById("__nuxt");
  if (root && root.childElementCount === 0) {
    window.location.reload();
  }
}, 500)"#;

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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_page_load(|webview, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            let _ = webview.eval(RELOAD_IF_BLANK_JS);
        })
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
            get_mod,
            list_mod_dependencies,
            get_user_profile,
            list_user_mods,
            request_email_code,
            verify_email_code,
            auth_status,
            logout,
            game_path_status,
            set_game_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
