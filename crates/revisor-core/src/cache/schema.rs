use anyhow::{Context, Result};
use rusqlite::Connection;

const CURRENT_VERSION: u32 = 1;

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    // Create version table if not exists
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY);"
    ).context("Failed to create schema_version table")?;

    let version: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 1 {
        migrate_v1(conn)?;
    }

    Ok(())
}

fn migrate_v1(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    tx.execute_batch(
        "
        -- Saved/favorite repositories
        CREATE TABLE IF NOT EXISTS saved_repos (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            owner       TEXT NOT NULL,
            repo        TEXT NOT NULL,
            added_at    TEXT NOT NULL DEFAULT (datetime('now')),
            last_used   TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(owner, repo)
        );

        -- Cached pull requests
        CREATE TABLE IF NOT EXISTS cached_pull_requests (
            id              INTEGER PRIMARY KEY,
            owner           TEXT NOT NULL,
            repo            TEXT NOT NULL,
            number          INTEGER NOT NULL,
            data_json       TEXT NOT NULL,
            etag            TEXT,
            fetched_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(owner, repo, number)
        );
        CREATE INDEX IF NOT EXISTS idx_pr_repo ON cached_pull_requests(owner, repo);

        -- Cached PR files (metadata + parsed diff as msgpack blob)
        CREATE TABLE IF NOT EXISTS cached_pr_files (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            owner           TEXT NOT NULL,
            repo            TEXT NOT NULL,
            pr_number       INTEGER NOT NULL,
            filename        TEXT NOT NULL,
            data_blob       BLOB,
            fetched_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(owner, repo, pr_number, filename)
        );
        CREATE INDEX IF NOT EXISTS idx_pr_files ON cached_pr_files(owner, repo, pr_number);

        -- Cached review comments
        CREATE TABLE IF NOT EXISTS cached_comments (
            id              INTEGER PRIMARY KEY,
            owner           TEXT NOT NULL,
            repo            TEXT NOT NULL,
            pr_number       INTEGER NOT NULL,
            data_json       TEXT NOT NULL,
            fetched_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_comments_pr ON cached_comments(owner, repo, pr_number);

        -- Local review state (not synced to provider)
        CREATE TABLE IF NOT EXISTS reviewed_files (
            owner       TEXT NOT NULL,
            repo        TEXT NOT NULL,
            pr_number   INTEGER NOT NULL,
            file_path   TEXT NOT NULL,
            reviewed_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (owner, repo, pr_number, file_path)
        );

        -- App settings (non-sensitive)
        CREATE TABLE IF NOT EXISTS settings (
            key     TEXT PRIMARY KEY,
            value   TEXT NOT NULL
        );

        -- Cache ETags for conditional requests
        CREATE TABLE IF NOT EXISTS cache_etags (
            url         TEXT PRIMARY KEY,
            etag        TEXT NOT NULL,
            fetched_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        INSERT OR REPLACE INTO schema_version (version) VALUES (1);
        ",
    )?;

    tx.commit().context("Failed to commit v1 migration")?;
    Ok(())
}
