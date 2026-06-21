use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Greenfield schema for `~/.ire/workspaces/<id>/local.db`. New workspaces only — no versioned
/// migrations. The two tables hold local-only operational state: detached
/// experiment rows and chat sessions. (Resources are file-based; the git-tracked
/// experiment *display* record lives in `ire.json`.)
const SCHEMA: &str = "
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
    session_id TEXT NOT NULL,
    tab_id TEXT NOT NULL DEFAULT 'main'
);

CREATE INDEX IF NOT EXISTS idx_experiments_status ON experiments(status);
CREATE INDEX IF NOT EXISTS idx_experiments_started ON experiments(started_at DESC);

CREATE TABLE IF NOT EXISTS chat_sessions (
    session_uuid      TEXT PRIMARY KEY,
    tab_label         TEXT NOT NULL,
    provider          TEXT NOT NULL,
    model             TEXT NOT NULL,
    started_at        TEXT NOT NULL,
    ended_at          TEXT NOT NULL,
    message_count     INTEGER NOT NULL,
    first_user_msg    TEXT,
    messages_json     TEXT NOT NULL,
    claude_session_id TEXT,
    codex_thread_id   TEXT
);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_ended ON chat_sessions(ended_at DESC);
";

/// Create the local DB tables if they don't already exist.
pub fn run(home_data_dir: &Path) -> Result<()> {
    let db_path = home_data_dir.join("local.db");
    let conn = Connection::open(&db_path).with_context(|| format!("open {}", db_path.display()))?;
    conn.execute_batch(SCHEMA).context("create schema")?;
    Ok(())
}
