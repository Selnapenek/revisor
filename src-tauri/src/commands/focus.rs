use crate::{focus, keyring};

#[tauri::command]
pub fn focus_has_key() -> Result<bool, String> {
    keyring::get_focus_key()
        .map(|key| key.is_some())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn focus_save_key(key: String) -> Result<(), String> {
    keyring::store_focus_key(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn focus_remove_key() -> Result<(), String> {
    keyring::delete_focus_key().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_focus_files(
    snapshot: focus::FocusSnapshot,
) -> Result<focus::FocusResult, String> {
    let key = keyring::get_focus_key()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Save a TypeSafe API key before analysis".to_string())?;
    Ok(focus::analyze(snapshot, key).await)
}
