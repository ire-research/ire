use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

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

const MIGRATION_2: &str = "
ALTER TABLE experiments ADD COLUMN tab_id TEXT NOT NULL DEFAULT 'main';
";

const MIGRATION_3: &str = "
ALTER TABLE resources ADD COLUMN source_type TEXT NOT NULL DEFAULT 'url';
ALTER TABLE resources ADD COLUMN source_label TEXT;
UPDATE resources SET source_label = url WHERE source_label IS NULL;
";

pub fn run(ire_dir: &Path) -> Result<()> {
    let db_path = ire_dir.join("wiki/local.db");
    let conn = Connection::open(&db_path).with_context(|| format!("open {}", db_path.display()))?;
    conn.execute_batch(MIGRATION_1).context("run migration 1")?;

    // Ensure schema_migrations tracking is seeded before migration 2 check.
    let _ = conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
        params![chrono::Local::now().to_rfc3339()],
    );

    let v2_applied: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !v2_applied {
        conn.execute_batch(MIGRATION_2).context("run migration 2")?;
        conn.execute(
            "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (2, ?1)",
            params![chrono::Local::now().to_rfc3339()],
        )
        .context("record migration 2")?;
    }

    let v3_applied: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 3",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !v3_applied {
        conn.execute_batch(MIGRATION_3).context("run migration 3")?;
        conn.execute(
            "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (3, ?1)",
            params![chrono::Local::now().to_rfc3339()],
        )
        .context("record migration 3")?;
    }

    Ok(())
}
