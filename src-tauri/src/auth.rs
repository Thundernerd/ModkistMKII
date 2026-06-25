use serde::Serialize;
use tauri::{AppHandle, State};

use crate::modio_client::{format_modio_error, ModioState};

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
    let client = state.get_base_client()?;
    let response = client
        .request_code(&email)
        .await
        .map_err(format_modio_error)?;
    let message = response.data().await.map_err(|e| e.to_string())?;
    Ok(message.message)
}

#[tauri::command]
pub async fn verify_email_code(
    app: AppHandle,
    state: State<'_, ModioState>,
    code: String,
) -> Result<AuthUser, String> {
    let base = state.get_base_client()?;
    let response = base
        .request_token(&code)
        .await
        .map_err(format_modio_error)?;
    let token = response.data().await.map_err(|e| e.to_string())?;

    let authed = base.with_token(token.value.clone());
    let user_response = authed
        .get_authenticated_user()
        .await
        .map_err(format_modio_error)?;
    let user = user_response.data().await.map_err(|e| e.to_string())?;

    state.set_session(&app, token.value, user.username.clone())?;

    log::info!("Signed in as {}", user.username);
    Ok(AuthUser {
        username: user.username,
        profile_url: user.profile_url.to_string(),
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
