use crate::{keyring, rank_findings};

#[tauri::command]
pub async fn rank_ai_findings(
    snapshot: rank_findings::RankingSnapshot,
) -> Result<rank_findings::RankingResult, String> {
    let key = keyring::get_focus_key()
        .map_err(|e| e.to_string())?
        .ok_or("Save a TypeSafe API key before ranking")?;
    rank_findings::rank(snapshot, key).await
}
