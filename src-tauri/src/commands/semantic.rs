use crate::{keyring, semantic};

#[tauri::command]
pub async fn search_semantic_diff(
    snapshot: semantic::SemanticSnapshot,
    query: String,
) -> Result<semantic::SemanticResult, String> {
    let key = keyring::get_focus_key()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Save a TypeSafe API key before search".to_string())?;
    semantic::search(snapshot, query, key).await
}
