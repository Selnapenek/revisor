use crate::provider::types::{PaginatedResult, PrState, PullRequest};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_pull_requests(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    pr_state: String,
    page: u32,
    per_page: u32,
) -> Result<PaginatedResult<PullRequest>, String> {
    let guard = state.provider.read().await;
    let provider = guard
        .as_ref()
        .ok_or_else(|| "Not authenticated".to_string())?;

    let state_filter = match pr_state.as_str() {
        "closed" => PrState::Closed,
        "all" => PrState::All,
        _ => PrState::Open,
    };

    provider
        .list_pull_requests(&owner, &repo, state_filter, page, per_page)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pull_request(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PullRequest, String> {
    let guard = state.provider.read().await;
    let provider = guard
        .as_ref()
        .ok_or_else(|| "Not authenticated".to_string())?;

    provider
        .get_pull_request(&owner, &repo, number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_rate_limit(
    state: State<'_, AppState>,
) -> Result<crate::provider::types::RateLimitInfo, String> {
    let guard = state.provider.read().await;
    let provider = guard
        .as_ref()
        .ok_or_else(|| "Not authenticated".to_string())?;

    Ok(provider.rate_limit_status())
}
