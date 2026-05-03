use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

const MIGRATION_1: &str = "
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS experiments (
    uuid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    command TEXT NOT NULL,
    working_dir TEXT NOT NULL,
    status TEXT NOT NULL,
    exit_code INTEGER,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    pid INTEGER,
    wake_prompt TEXT,
    session_id TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_experiments_status ON experiments(status);
CREATE INDEX IF NOT EXISTS idx_experiments_started ON experiments(started_at DESC);

CREATE TABLE IF NOT EXISTS resources (
    url_sha256 TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    title TEXT,
    status TEXT NOT NULL,
    content_type TEXT,
    wiki_path TEXT,
    fetched_at TEXT,
    summarized_at TEXT,
    error TEXT
);

CREATE INDEX IF NOT EXISTS idx_resources_status ON resources(status);
";

pub fn run(ire_dir: &Path) -> Result<()> {
    let db_path = ire_dir.join("local.db");
    let conn =
        Connection::open(&db_path).with_context(|| format!("open {}", db_path.display()))?;
    conn.execute_batch(MIGRATION_1)
        .context("run migration 1")?;
    Ok(())
}
