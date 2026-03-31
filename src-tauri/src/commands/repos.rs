use crate::cache::queries;
use crate::provider::types::{PaginatedResult, RepoInfo};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_saved_repos(state: State<'_, AppState>) -> Result<Vec<(String, String)>, String> {
    let cache = state.cache.lock().await;
    queries::get_saved_repos(cache.conn()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_repo(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
) -> Result<(), String> {
    let cache = state.cache.lock().await;
    queries::save_repo(cache.conn(), &owner, &repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_repo(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
) -> Result<(), String> {
    let cache = state.cache.lock().await;
    queries::remove_repo(cache.conn(), &owner, &repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_user_repos(
    state: State<'_, AppState>,
    page: u32,
    per_page: u32,
) -> Result<PaginatedResult<RepoInfo>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;
    provider
        .list_user_repos(page, per_page)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_repos(
    state: State<'_, AppState>,
    query: String,
    page: u32,
    per_page: u32,
) -> Result<PaginatedResult<RepoInfo>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;
    provider
        .search_repos(&query, page, per_page)
        .await
        .map_err(|e| e.to_string())
}
