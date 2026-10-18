use crate::{keyring, source_snapshot};

#[tauri::command]
pub async fn get_pr_head_source_snapshot(
    owner: String,
    repo: String,
    head_sha: String,
) -> Result<source_snapshot::SourceSnapshot, String> {
    let token = keyring::get_token()
        .map_err(|_| "Could not read GitHub token")?
        .ok_or("Sign in to GitHub first")?;
    source_snapshot::load(&owner, &repo, &head_sha, &token).await
}
