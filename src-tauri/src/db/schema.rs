use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use rusqlite_migration::{Migrations, SchemaVersion, M};

/// Versioned schema for `~/.ire/workspaces/<id>/local.db`, tracked via SQLite's
/// `user_version` (see the `rusqlite_migration` crate). The two tables hold
/// local-only operational state: detached experiment rows and chat sessions.
/// An experiment's goal/context is not among them — it lives only in its
/// `EXPERIMENT.md`, which is what survives a clone or a cleared database.
/// (Resources are file-based; the git-tracked experiment record lives in
/// `.ire/experiments/<NNN>-<slug>/EXPERIMENT.md`, which owns status.)
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
        M::up("ALTER TABLE experiments ADD COLUMN record_dir TEXT;")
            .comment("remember each experiment's git-tracked record folder so status transitions can find it"),
        M::up("ALTER TABLE experiments DROP COLUMN wake_prompt;")
            .comment("drop wake_prompt: EXPERIMENT.md's Goal & context section is the only copy"),
        M::up(
            "DROP INDEX IF EXISTS idx_experiments_status;
            ALTER TABLE experiments DROP COLUMN status;
            ALTER TABLE experiments DROP COLUMN exit_code;
            ALTER TABLE experiments DROP COLUMN ended_at;",
        )
        .comment("drop status/exit_code/ended_at: EXPERIMENT.md's record is the only copy"),
    ])
}

/// The last version that still carries the fields the record backfill reads
/// (`wake_prompt`, then `status`/`exit_code`/`ended_at`). Each backfill runs
/// against a database frozen at this version before the following migration
/// deletes what it just read.
const BEFORE_BACKFILL_SOURCE_DROP: usize = 4;

/// Migrate the local DB to the latest schema version, creating it if needed.
pub fn run(home_data_dir: &Path) -> Result<()> {
    migrate(home_data_dir, None)
}

/// Migrate only as far as the schema the record backfill needs to read.
///
/// On workspace open this runs first, then `experiments::migrate`, then
/// [`run`]. Running the whole thing up front would drop `wake_prompt` before
/// the backfill could copy an experiment's goal into its record, and that text
/// exists nowhere else.
///
/// A database already past this version is left alone: migrations only move
/// forward, so this is a no-op rather than an error.
pub fn run_pre_backfill(home_data_dir: &Path) -> Result<()> {
    migrate(home_data_dir, Some(BEFORE_BACKFILL_SOURCE_DROP))
}

fn migrate(home_data_dir: &Path, version: Option<usize>) -> Result<()> {
    let db_path = home_data_dir.join("local.db");
    let mut conn =
        Connection::open(&db_path).with_context(|| format!("open {}", db_path.display()))?;
    let Some(target) = version else {
        return migrations()
            .to_latest(&mut conn)
            .context("apply schema migrations");
    };
    // `to_version` refuses to go backwards, so only call it when there is
    // something to apply.
    let current = migrations()
        .current_version(&conn)
        .context("read schema version")?;
    if current < SchemaVersion::Inside(std::num::NonZeroUsize::new(target).unwrap()) {
        migrations()
            .to_version(&mut conn, target)
            .context("apply schema migrations")?;
    }
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

    #[test]
    fn wake_prompt_is_gone_after_migrating() {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path()).unwrap();
        let conn = Connection::open(tmp.path().join("local.db")).unwrap();
        let has_column: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM pragma_table_info('experiments') WHERE name = 'wake_prompt')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!has_column, "wake_prompt survived the migration");
    }

    /// Issue #120: status/exit_code/ended_at are vestigial now that
    /// EXPERIMENT.md is the source of truth for a run's outcome. A fresh
    /// migration must not leave any of them on the experiments table.
    #[test]
    fn status_columns_are_gone_after_migrating() {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path()).unwrap();
        let conn = Connection::open(tmp.path().join("local.db")).unwrap();
        for col in ["status", "exit_code", "ended_at"] {
            let has_column: bool = conn
                .query_row(
                    "SELECT EXISTS (SELECT 1 FROM pragma_table_info('experiments') WHERE name = ?1)",
                    params![col],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(!has_column, "{col} survived the migration");
        }
    }

    /// Issue #120: dropping the status columns reuses the same two-phase
    /// schema pass #114 introduced for `wake_prompt` — `run_pre_backfill` must
    /// still stop before migration 5 (the status-column drop) so
    /// `experiments::migrate::run` can still read `status`/`exit_code`/
    /// `ended_at` off a legacy row before they're gone for good.
    #[test]
    fn pre_backfill_still_stops_before_the_status_column_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("local.db");

        // Seed a legacy row the way a pre-#120 local.db would have it, with
        // the eventually-doomed status columns populated.
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE experiments (
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
                );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO experiments \
                 (uuid, name, command, working_dir, status, exit_code, started_at, ended_at, session_id, tab_id) \
                 VALUES ('e1', 'exp', 'run.py', '/tmp', 'completed', 0, 't0', 't1', 's1', 'main')",
                [],
            )
            .unwrap();
        }

        // Running only run_pre_backfill must leave the status columns intact,
        // so a caller in experiments::migrate::run between the two schema
        // passes can still read them.
        run_pre_backfill(tmp.path()).unwrap();
        let conn = Connection::open(&db_path).unwrap();
        let status: String = conn
            .query_row("SELECT status FROM experiments WHERE uuid = 'e1'", [], |r| r.get(0))
            .expect("status column must still be readable after run_pre_backfill");
        assert_eq!(status, "completed");

        // Finishing the pass then drops them, same as `run` alone would.
        run(tmp.path()).unwrap();
        let conn = Connection::open(&db_path).unwrap();
        let has_status: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM pragma_table_info('experiments') WHERE name = 'status')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!has_status, "status must be dropped once the full pass has run");
    }
}
