use crate::keyring;
use crate::provider::github::GithubProvider;
use crate::provider::types::ProviderUser;
use crate::provider::GitProvider;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_stored_token() -> Result<Option<String>, String> {
    keyring::get_token().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_and_store_token(
    state: State<'_, AppState>,
    token: String,
) -> Result<ProviderUser, String> {
    // Create provider and validate
    let provider = GithubProvider::new(token.clone()).map_err(|e| e.to_string())?;
    let user = provider.validate_token().await.map_err(|e| e.to_string())?;

    // Store token in keyring
    keyring::store_token(&token).map_err(|e| e.to_string())?;

    // Store provider in app state
    let mut guard = state.provider.write().await;
    *guard = Some(Box::new(provider));

    Ok(user)
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    keyring::delete_token().map_err(|e| e.to_string())?;
    let mut guard = state.provider.write().await;
    *guard = None;
    Ok(())
}

#[tauri::command]
pub async fn restore_session(state: State<'_, AppState>) -> Result<Option<ProviderUser>, String> {
    let token = match keyring::get_token().map_err(|e| e.to_string())? {
        Some(t) => t,
        None => return Ok(None),
    };

    let provider = GithubProvider::new(token).map_err(|e| e.to_string())?;
    match provider.validate_token().await {
        Ok(user) => {
            let mut guard = state.provider.write().await;
            *guard = Some(Box::new(provider));
            Ok(Some(user))
        }
        Err(_) => {
            // Token is invalid, clean it up
            let _ = keyring::delete_token();
            Ok(None)
        }
    }
}
