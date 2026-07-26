use crate::provider::types::{IssueComment, NewComment, PrReview, ReviewComment, ThreadResolutionInfo};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn update_comment(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    comment_id: u64,
    body: String,
) -> Result<ReviewComment, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .update_comment(&owner, &repo, comment_id, &body)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_comment(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    comment_id: u64,
) -> Result<(), String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .delete_comment(&owner, &repo, comment_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pr_comments(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<ReviewComment>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .get_pr_comments(&owner, &repo, number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_issue_comments(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<IssueComment>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .get_issue_comments(&owner, &repo, number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pr_reviews(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<PrReview>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .get_pr_reviews(&owner, &repo, number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_comment(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
    path: String,
    line: u64,
    side: String,
    body: String,
    commit_id: String,
) -> Result<ReviewComment, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    let params = NewComment {
        path,
        line,
        side,
        body,
        commit_id,
    };

    provider
        .add_comment(&owner, &repo, number, params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reply_to_comment(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
    comment_id: u64,
    body: String,
) -> Result<ReviewComment, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .reply_to_comment(&owner, &repo, number, comment_id, &body)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_thread_resolutions(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<ThreadResolutionInfo>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .get_thread_resolutions(&owner, &repo, number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resolve_thread(
    state: State<'_, AppState>,
    thread_node_id: String,
) -> Result<(), String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .resolve_thread(&thread_node_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unresolve_thread(
    state: State<'_, AppState>,
    thread_node_id: String,
) -> Result<(), String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .unresolve_thread(&thread_node_id)
        .await
        .map_err(|e| e.to_string())
}
