use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

/// Versioned schema for `~/.ire/workspaces/<id>/local.db`, tracked via SQLite's
/// `user_version` (see the `rusqlite_migration` crate). The two tables hold
/// local-only operational state: detached experiment rows and chat sessions.
/// (Resources are file-based; the git-tracked experiment *display* record
/// lives in `ire.json`.)
///
/// Every migration's SQL must be safe to run both on a brand-new database and
/// on a pre-migration one: this schema shipped for a long time as a single
/// `CREATE TABLE IF NOT EXISTS` batch applied on every launch with no version
/// tracking, so any `local.db` created before this file introduced real
/// migrations is sitting at the (untracked) equivalent of version 0 with
/// tables already in place. `IF NOT EXISTS` in migration 1 makes it a no-op
/// for those; a fresh database gets its tables created for the first time.
fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE IF NOT EXISTS experiments (
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
            CREATE INDEX IF NOT EXISTS idx_chat_sessions_ended ON chat_sessions(ended_at DESC);",
        )
        .comment("baseline schema: experiments + chat_sessions"),
        M::up(
            "CREATE TABLE IF NOT EXISTS chat_resume_ids (
                session_uuid TEXT NOT NULL,
                provider     TEXT NOT NULL,
                resume_id    TEXT NOT NULL,
                PRIMARY KEY (session_uuid, provider)
            );

            INSERT OR IGNORE INTO chat_resume_ids (session_uuid, provider, resume_id)
            SELECT session_uuid, provider,
                   CASE WHEN provider = 'codex' THEN codex_thread_id ELSE claude_session_id END
            FROM chat_sessions
            WHERE (CASE WHEN provider = 'codex' THEN codex_thread_id ELSE claude_session_id END) IS NOT NULL;

            ALTER TABLE chat_sessions DROP COLUMN claude_session_id;
            ALTER TABLE chat_sessions DROP COLUMN codex_thread_id;",
        )
        .comment("move resume ids off fixed per-provider columns into chat_resume_ids(session_uuid, provider)"),
    ])
}

/// Migrate the local DB to the latest schema version, creating it if needed.
pub fn run(home_data_dir: &Path) -> Result<()> {
    let db_path = home_data_dir.join("local.db");
    let mut conn =
        Connection::open(&db_path).with_context(|| format!("open {}", db_path.display()))?;
    migrations()
        .to_latest(&mut conn)
        .context("apply schema migrations")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn migrations_are_valid() {
        migrations().validate().unwrap();
    }

    #[test]
    fn backfill_recovers_legacy_resume_ids_on_upgraded_db() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("local.db");

        // Simulate a pre-migration DB: chat_sessions with the two legacy
        // columns, populated as if by the old upsert_chat_resume_id, and no
        // user_version set (as every local.db predating this file was).
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE chat_sessions (
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
                );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO chat_sessions \
                 (session_uuid, tab_label, provider, model, started_at, ended_at, message_count, messages_json, claude_session_id) \
                 VALUES ('s1', 'tab', 'claude', 'claude-sonnet-5', 't0', 't1', 1, '[]', 'claude-abc')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO chat_sessions \
                 (session_uuid, tab_label, provider, model, started_at, ended_at, message_count, messages_json, codex_thread_id) \
                 VALUES ('s2', 'tab', 'codex', 'gpt-5.4', 't0', 't1', 1, '[]', 'codex-xyz')",
                [],
            )
            .unwrap();
        }

        run(tmp.path()).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let get_resume = |session_uuid: &str, provider: &str| -> Option<String> {
            conn.query_row(
                "SELECT resume_id FROM chat_resume_ids WHERE session_uuid = ?1 AND provider = ?2",
                params![session_uuid, provider],
                |r| r.get(0),
            )
            .ok()
        };
        assert_eq!(get_resume("s1", "claude"), Some("claude-abc".to_string()));
        assert_eq!(get_resume("s2", "codex"), Some("codex-xyz".to_string()));

        // Legacy columns are gone post-migration.
        let has_legacy_column: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM pragma_table_info('chat_sessions') WHERE name = 'claude_session_id')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!has_legacy_column);

        // Idempotent: re-running against an already-migrated DB is a no-op, not an error.
        run(tmp.path()).unwrap();
        assert_eq!(get_resume("s1", "claude"), Some("claude-abc".to_string()));
    }

    #[test]
    fn fresh_db_migrates_cleanly() {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path()).unwrap();
        run(tmp.path()).unwrap(); // re-running an already-latest DB must not error
    }
}
