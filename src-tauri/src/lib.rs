#[cfg(not(unix))]
mod wine_prefix {
    use std::path::Path;

    use serde::Serialize;

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WineWinhttpStatus {
        pub state: String,
        pub message: Option<String>,
        pub prefix_label: Option<String>,
    }

    pub fn configure_winhttp_override(_game_dir: &Path) -> WineWinhttpStatus {
        WineWinhttpStatus {
            state: "notApplicable".into(),
            message: None,
            prefix_label: None,
        }
    }
}

#[cfg(unix)]
mod wine_prefix;

mod app_settings;
mod auth;
mod bepinex;
mod game_detect;
mod game_launch;
mod game_path;
mod game_process;
mod logging;
mod mod_api_cache;
mod mod_download;
mod mod_folder;
mod modio_env;
mod mod_install;
pub mod modio_api;
mod modio_client;
mod profiles;
mod sentry_init;
mod subscription_sync_settings;
mod zip_extract;

use app_settings::{
    get_app_settings, get_ignore_bepinex_version_warning_enabled, remember_skip_sign_in,
    remember_ignore_bepinex_version_warning, set_auto_update_mods,
    set_ignore_bepinex_version_warning,
};
use auth::{auth_status, logout, request_email_code, verify_email_code};
use bepinex::{bepinex_status, install_bepinex, reinstall_bepinex, verify_bepinex};
use game_detect::detect_game_paths_command;
use game_launch::launch_game;
use game_path::{game_path_status, set_game_path};
use game_process::game_running_status;
use logging::log_directory_path;
use mod_install::{
    cancel_subscription_sync, get_mod_install_state, install_mod, list_installed_mod_records,
    list_installed_mods, refresh_installed_mods, sync_subscribed_mods, uninstall_mod,
};
use profiles::{
    create_profile, delete_profile, get_active_profile, list_profiles,
    logout_requires_profile_selection_command, rename_profile, switch_profile,
};
use subscription_sync_settings::{
    list_failed_sync_mods_command, set_failed_sync_mod_ignored, unsubscribe_failed_sync_mod,
};
use modio_client::{
    get_mod, get_mod_tag_options, get_user_profile, list_mod_dependencies, list_mod_files,
    list_mods, list_user_mods, modio_status, ModioState,
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

    let _sentry_guard = sentry_init::init();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());

    if let Some(client) = sentry_init::client() {
        builder = builder.plugin(tauri_plugin_sentry::init_with_no_injection(&client));
    }

    builder
        .on_page_load(|webview, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            let _ = webview.eval(RELOAD_IF_BLANK_JS);
        })
        .setup(|app| {
            match logging::init(app.handle()) {
                Ok(log_dir) => {
                    log::info!("Modkist starting");
                    log::info!("Writing logs to {}", log_dir.display());
                    if sentry_init::is_enabled() {
                        log::info!("Sentry error reporting enabled");
                    }
                }
                Err(error) => {
                    eprintln!("Failed to initialize file logging: {error}");
                }
            }

            let state = ModioState::from_env();
            if let Err(error) = state.restore_from_store(app.handle()) {
                log::warn!("Failed to restore mod.io session: {error}");
            } else if state.auth_status().logged_in {
                log::info!(
                    "Restored mod.io session for {}",
                    state.auth_status().username.as_deref().unwrap_or("user")
                );
            }
            state.load_persisted_cache(app.handle());
            app.manage(state);
            log::info!("Application setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            log_directory_path,
            modio_status,
            get_mod_tag_options,
            list_mods,
            get_mod,
            list_mod_files,
            list_mod_dependencies,
            get_user_profile,
            list_user_mods,
            request_email_code,
            verify_email_code,
            auth_status,
            logout,
            game_path_status,
            set_game_path,
            detect_game_paths_command,
            get_app_settings,
            get_ignore_bepinex_version_warning_enabled,
            set_auto_update_mods,
            set_ignore_bepinex_version_warning,
            remember_ignore_bepinex_version_warning,
            remember_skip_sign_in,
            game_running_status,
            launch_game,
            bepinex_status,
            verify_bepinex,
            install_bepinex,
            reinstall_bepinex,
            list_installed_mods,
            list_installed_mod_records,
            refresh_installed_mods,
            get_mod_install_state,
            install_mod,
            cancel_subscription_sync,
            sync_subscribed_mods,
            uninstall_mod,
            list_profiles,
            get_active_profile,
            switch_profile,
            create_profile,
            delete_profile,
            rename_profile,
            logout_requires_profile_selection_command,
            list_failed_sync_mods_command,
            set_failed_sync_mod_ignored,
            unsubscribe_failed_sync_mod,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
