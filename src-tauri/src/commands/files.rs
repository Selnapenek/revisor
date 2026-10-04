use crate::diff::{fetch_pr_files, ProcessedFile};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_pr_files(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<ProcessedFile>, String> {
    fetch_pr_files(&*state.provider().await?, &owner, &repo, number).await
}

#[tauri::command]
pub async fn get_file_content(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    path: String,
    ref_: String,
) -> Result<Vec<String>, String> {
    let content = state
        .provider()
        .await?
        .get_file_content(&owner, &repo, &path, &ref_)
        .await
        .map_err(|e| e.to_string())?;
    Ok(content.split('\n').map(|s| s.to_string()).collect())
}
