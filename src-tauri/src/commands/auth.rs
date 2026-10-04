use crate::auth::{self, DeviceLoginInfo};
use crate::provider::types::ProviderUser;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn validate_and_store_token(
    state: State<'_, AppState>,
    token: String,
) -> Result<ProviderUser, String> {
    auth::validate_and_store_token(&state, token).await
}

#[tauri::command]
pub async fn start_github_login(state: State<'_, AppState>) -> Result<DeviceLoginInfo, String> {
    auth::start_github_login(&state).await
}

#[tauri::command]
pub async fn wait_for_github_login(
    state: State<'_, AppState>,
    attempt_id: String,
) -> Result<ProviderUser, String> {
    auth::wait_for_github_login(&state, attempt_id).await
}

#[tauri::command]
pub async fn cancel_github_login(
    state: State<'_, AppState>,
    attempt_id: String,
) -> Result<(), String> {
    auth::cancel_github_login(&state, attempt_id).await
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    auth::logout(&state).await
}

#[tauri::command]
pub async fn restore_session(state: State<'_, AppState>) -> Result<Option<ProviderUser>, String> {
    auth::restore_session(&state).await
}
