use crate::commands::files::{process_pr_files, ProcessedFile};
use crate::provider::types::Commit;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_pr_commits(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<Commit>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    provider
        .get_pr_commits(&owner, &repo, number)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_compare_files(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    base: String,
    head: String,
) -> Result<Vec<ProcessedFile>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    let files = provider
        .get_compare_files(&owner, &repo, &base, &head)
        .await
        .map_err(|e| e.to_string())?;

    Ok(process_pr_files(files))
}
