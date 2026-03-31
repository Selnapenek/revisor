use anyhow::{Context, Result};
use rusqlite::{params, Connection};

// --- Settings ---

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let result = stmt
        .query_row(params![key], |row| row.get(0))
        .ok();
    Ok(result)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

// --- Saved Repos ---

pub fn get_saved_repos(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT owner, repo FROM saved_repos ORDER BY last_used DESC"
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to read saved repos")?;
    Ok(rows)
}

pub fn save_repo(conn: &Connection, owner: &str, repo: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO saved_repos (owner, repo) VALUES (?1, ?2)
         ON CONFLICT(owner, repo) DO UPDATE SET last_used = datetime('now')",
        params![owner, repo],
    )?;
    Ok(())
}

pub fn remove_repo(conn: &Connection, owner: &str, repo: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM saved_repos WHERE owner = ?1 AND repo = ?2",
        params![owner, repo],
    )?;
    Ok(())
}

// --- Reviewed Files ---

pub fn get_reviewed_files(
    conn: &Connection,
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT file_path FROM reviewed_files
         WHERE owner = ?1 AND repo = ?2 AND pr_number = ?3
         ORDER BY reviewed_at"
    )?;
    let rows = stmt
        .query_map(params![owner, repo, pr_number as i64], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to read reviewed files")?;
    Ok(rows)
}

pub fn mark_file_reviewed(
    conn: &Connection,
    owner: &str,
    repo: &str,
    pr_number: u64,
    file_path: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO reviewed_files (owner, repo, pr_number, file_path)
         VALUES (?1, ?2, ?3, ?4)",
        params![owner, repo, pr_number as i64, file_path],
    )?;
    Ok(())
}

pub fn unmark_file_reviewed(
    conn: &Connection,
    owner: &str,
    repo: &str,
    pr_number: u64,
    file_path: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM reviewed_files
         WHERE owner = ?1 AND repo = ?2 AND pr_number = ?3 AND file_path = ?4",
        params![owner, repo, pr_number as i64, file_path],
    )?;
    Ok(())
}
