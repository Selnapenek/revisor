use crate::keyring;
use crate::provider::github::GithubProvider;
use crate::provider::types::ProviderUser;
use crate::provider::GitProvider;
use crate::AppState;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests {
    use super::normalize_pat;

    #[test]
    fn rejects_empty_pat_and_trims_valid_pat() {
        assert!(normalize_pat(" \n ").is_err());
        assert_eq!(normalize_pat("  ghp_example  ").unwrap(), "ghp_example");
    }
}

fn normalize_pat(token: &str) -> Result<&str, String> {
    let token = token.trim();
    if token.is_empty() {
        Err("Personal access token is required.".into())
    } else {
        Ok(token)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLoginInfo {
    pub attempt_id: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

pub async fn validate_and_store_token(
    state: &AppState,
    token: String,
) -> Result<ProviderUser, String> {
    let token = normalize_pat(&token)?;
    let provider = GithubProvider::new(token.to_owned()).map_err(|e| e.to_string())?;
    let user = provider.validate_token().await.map_err(|e| e.to_string())?;
    keyring::store_token(token).map_err(|e| e.to_string())?;
    let mut guard = state.provider.write().await;
    *guard = Some(Box::new(provider));
    Ok(user)
}

pub async fn start_github_login(state: &AppState) -> Result<DeviceLoginInfo, String> {
    let client_id = option_env!("REVISOR_GITHUB_CLIENT_ID")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "GitHub OAuth is not configured. Set REVISOR_GITHUB_CLIENT_ID and rebuild Revisor."
                .to_string()
        })?;
    let attempt_id = format!(
        "{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let mut pending = state.pending_login.lock().await;
    if let Some(old) = pending.take() {
        old.cancelled.store(true, Ordering::SeqCst);
    }
    let login = crate::oauth::begin(client_id, attempt_id.clone())
        .await
        .map_err(|e| e.to_string())?;
    let info = DeviceLoginInfo {
        attempt_id,
        user_code: login.user_code.clone(),
        verification_uri: login.verification_uri.clone(),
        expires_in: login
            .expires_at
            .saturating_duration_since(std::time::Instant::now())
            .as_secs(),
    };
    *pending = Some(crate::PendingLogin {
        login,
        cancelled: Arc::new(AtomicBool::new(false)),
        polling: false,
    });
    Ok(info)
}

pub async fn wait_for_github_login(
    state: &AppState,
    attempt_id: String,
) -> Result<ProviderUser, String> {
    let client_id = option_env!("REVISOR_GITHUB_CLIENT_ID")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "GitHub OAuth is not configured. Set REVISOR_GITHUB_CLIENT_ID and rebuild Revisor."
                .to_string()
        })?;
    let (login, cancelled) = {
        let mut pending = state.pending_login.lock().await;
        let pending = pending
            .as_mut()
            .filter(|p| p.login.attempt_id == attempt_id)
            .ok_or_else(|| "GitHub sign-in is no longer active. Start again.".to_string())?;
        if pending.polling {
            return Err("GitHub sign-in is already in progress.".into());
        }
        pending.polling = true;
        (pending.login.clone(), pending.cancelled.clone())
    };
    let token = crate::oauth::poll(client_id, &login, cancelled.clone())
        .await
        .map_err(|e| e.to_string())?;
    if cancelled.load(Ordering::SeqCst) {
        return Err("GitHub sign-in was cancelled.".into());
    }
    let provider = GithubProvider::new(token.clone()).map_err(|e| e.to_string())?;
    let user = provider.validate_token().await.map_err(|e| e.to_string())?;
    if cancelled.load(Ordering::SeqCst) {
        return Err("GitHub sign-in was cancelled.".into());
    }
    let mut pending = state.pending_login.lock().await;
    if cancelled.load(Ordering::SeqCst)
        || !pending
            .as_ref()
            .is_some_and(|p| p.login.attempt_id == attempt_id)
    {
        return Err("GitHub sign-in was cancelled.".into());
    }
    keyring::store_token(&token).map_err(|e| e.to_string())?;
    let mut guard = state.provider.write().await;
    *guard = Some(Box::new(provider));
    pending.take();
    Ok(user)
}

pub async fn cancel_github_login(
    state: &AppState,
    attempt_id: String,
) -> Result<(), String> {
    let mut pending = state.pending_login.lock().await;
    if pending
        .as_ref()
        .is_some_and(|p| p.login.attempt_id == attempt_id)
    {
        if let Some(login) = pending.take() {
            login.cancelled.store(true, Ordering::SeqCst);
        }
    }
    Ok(())
}

pub async fn logout(state: &AppState) -> Result<(), String> {
    if let Some(login) = state.pending_login.lock().await.take() {
        login.cancelled.store(true, Ordering::SeqCst);
    }
    keyring::delete_token().map_err(|e| e.to_string())?;
    let mut guard = state.provider.write().await;
    *guard = None;
    Ok(())
}

pub async fn restore_session(state: &AppState) -> Result<Option<ProviderUser>, String> {
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
        Err(e) if e.to_string().contains("401") || e.to_string().contains("Unauthorized") => {
            // Token is invalid, clean it up
            let _ = keyring::delete_token();
            Ok(None)
        }
        Err(e) => Err(e.to_string()),
    }
}
