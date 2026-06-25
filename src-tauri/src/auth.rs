use serde::Serialize;
use tauri::{AppHandle, State};

use crate::modio_client::{format_api_error, ModioState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub username: String,
    pub profile_url: String,
}

#[tauri::command]
pub async fn request_email_code(
    state: State<'_, ModioState>,
    email: String,
) -> Result<String, String> {
    log::info!("Requesting mod.io login code");
    let api = state.api()?;
    let message = api.request_email_code(&email).await.map_err(format_api_error)?;
    Ok(message.message)
}

#[tauri::command]
pub async fn verify_email_code(
    app: AppHandle,
    state: State<'_, ModioState>,
    code: String,
) -> Result<AuthUser, String> {
    let api = state.api()?;
    let token = api.exchange_email_code(&code).await.map_err(format_api_error)?;

    let user = api
        .get_authenticated_user(&token.access_token)
        .await
        .map_err(format_api_error)?;

    state.set_session(&app, token.access_token, user.username.clone())?;

    log::info!("Signed in as {}", user.username);
    Ok(AuthUser {
        username: user.username,
        profile_url: user.profile_url,
    })
}

#[tauri::command]
pub fn auth_status(state: State<'_, ModioState>) -> crate::modio_client::AuthStatus {
    state.auth_status()
}

#[tauri::command]
pub fn logout(app: AppHandle, state: State<'_, ModioState>) -> Result<(), String> {
    log::info!("Logging out");
    state.clear_session(&app)
}
