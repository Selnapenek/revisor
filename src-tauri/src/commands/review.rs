use crate::cache::queries;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_reviewed_files(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    pr_number: u64,
) -> Result<Vec<String>, String> {
    let cache = state.cache.lock().await;
    queries::get_reviewed_files(cache.conn(), &owner, &repo, pr_number)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mark_file_reviewed(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    pr_number: u64,
    file_path: String,
) -> Result<(), String> {
    let cache = state.cache.lock().await;
    queries::mark_file_reviewed(cache.conn(), &owner, &repo, pr_number, &file_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unmark_file_reviewed(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    pr_number: u64,
    file_path: String,
) -> Result<(), String> {
    let cache = state.cache.lock().await;
    queries::unmark_file_reviewed(cache.conn(), &owner, &repo, pr_number, &file_path)
        .map_err(|e| e.to_string())
}
