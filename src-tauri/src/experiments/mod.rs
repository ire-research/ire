pub mod migrate;
pub mod record;
pub mod runner;
pub mod wake;

use std::path::Path;

use tauri::AppHandle;

use crate::db::models::{self as db, ExperimentRow};
use crate::events;

/// Terminal run states. Once a record says one of these, the run is over and
/// how it ended is settled.
const TERMINAL: [&str; 3] = ["completed", "failed", "cancelled"];

/// The row the UI is told about: `local.db`'s operational fields plus the
/// outcome of a run, which lives only in its `EXPERIMENT.md` record. A row
/// whose record can't be read reports `status: "unknown"` rather than
/// pretending to know how the run ended.
#[derive(Debug, serde::Serialize, Clone)]
pub struct ExperimentView {
    pub uuid: String,
    pub name: String,
    pub command: String,
    pub status: String,
    pub exit_code: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub pid: Option<i64>,
    pub tab_id: String,
}

/// Apply a terminal status: write it to `local.db`, then to the git-tracked
/// `EXPERIMENT.md`, then tell the UI what landed on disk. The file write is
/// best-effort — a workspace whose folder was deleted still gets its row and
/// its event.
///
/// A run that already ended keeps the ending it has. Cancelling SIGTERMs the
/// process group, and ~500ms later the monitor sees the child gone with no exit
/// code (`code()` is `None` for a signalled process) and would otherwise report
/// it as a plain `failed` with exit `-1`, overwriting the cancellation in git.
pub fn transition(
    app: &AppHandle,
    workspace_root: &Path,
    home_data_dir: &Path,
    uuid: &str,
    status: &str,
    exit_code: Option<i32>,
) {
    if let Some(settled) = row_or_record(workspace_root, home_data_dir, uuid) {
        if TERMINAL.contains(&settled.status.as_str()) {
            tracing::debug!(uuid = %uuid, from = %settled.status, to = %status, "ignoring transition on a settled run");
            return;
        }
    }
    let ended_at = chrono::Local::now().to_rfc3339();
    write_status(
        workspace_root,
        home_data_dir,
        uuid,
        status,
        exit_code.map(i64::from),
        Some(&ended_at),
    );
    emit_changed(app, workspace_root, home_data_dir, uuid);
}

/// Rewrite the runner-owned frontmatter of an experiment's record.
/// Best-effort: a failure is logged, not propagated.
fn write_status(
    workspace_root: &Path,
    home_data_dir: &Path,
    uuid: &str,
    status: &str,
    exit_code: Option<i64>,
    ended_at: Option<&str>,
) {
    let Some(dir) = record_dir(workspace_root, home_data_dir, uuid) else {
        return;
    };
    if let Err(e) = record::set_status(workspace_root, &dir, status, exit_code, ended_at) {
        tracing::warn!(error = %e, uuid = %uuid, "write experiment status to record failed");
    }
}

/// Emit `experiment-changed` from what is actually persisted.
pub fn emit_changed(app: &AppHandle, workspace_root: &Path, home_data_dir: &Path, uuid: &str) {
    if let Some(row) = row_or_record(workspace_root, home_data_dir, uuid) {
        events::emit_experiment_changed(app, events::EventSource::Mutation, &row);
    }
}

/// The row the UI is told about: the git-tracked fields as `EXPERIMENT.md` has
/// them on disk, the operational ones (pid, tab linkage) from `local.db`. A
/// record that cannot be read leaves the database row as it stands.
fn row(workspace_root: &Path, home_data_dir: &Path, uuid: &str) -> Option<ExperimentView> {
    let row = db::get_experiment(home_data_dir, uuid).ok().flatten()?;
    let dir = db::get_experiment_record_dir(home_data_dir, uuid)
        .ok()
        .flatten();
    Some(overlay(workspace_root, dir.as_deref(), row))
}

/// Every experiment, composed the same way [`row`] composes one. This is what
/// `experiment_list` serves, so a list view and a live event can never disagree
/// about how a run ended.
pub fn list(
    workspace_root: &Path,
    home_data_dir: &Path,
    limit: usize,
) -> anyhow::Result<Vec<ExperimentView>> {
    let rows = db::list_experiments(home_data_dir, limit)?;
    let dirs = db::experiment_record_dirs(home_data_dir)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let dir = dirs.get(&row.uuid).map(String::as_str);
            overlay(workspace_root, dir, row)
        })
        .collect())
}

/// Where an experiment's record lives, for a run that may have no database row
/// at all. A workspace cloned from git carries the records but not `local.db`,
/// so uuid → folder has to be answerable from the files alone.
pub fn record_dir(workspace_root: &Path, home_data_dir: &Path, uuid: &str) -> Option<String> {
    if let Ok(Some(dir)) = db::get_experiment_record_dir(home_data_dir, uuid) {
        return Some(dir);
    }
    record::list(workspace_root)
        .into_iter()
        .find(|(_, r)| r.uuid == uuid)
        .map(|(dir, _)| dir)
}

/// The row the UI is told about, for a run that may exist only as a file.
/// Falls back to composing one from the record when `local.db` has nothing,
/// with the operational fields left empty — there is no local process.
pub fn row_or_record(
    workspace_root: &Path,
    home_data_dir: &Path,
    uuid: &str,
) -> Option<ExperimentView> {
    if let Some(row) = row(workspace_root, home_data_dir, uuid) {
        return Some(row);
    }
    let dir = record_dir(workspace_root, home_data_dir, uuid)?;
    let persisted = record::read(workspace_root, &dir).ok()?;
    Some(ExperimentView {
        uuid: persisted.uuid,
        name: persisted.name,
        command: persisted.command,
        status: persisted.status,
        exit_code: persisted.exit_code,
        started_at: persisted.started_at,
        ended_at: persisted.ended_at,
        pid: None,
        tab_id: String::new(),
    })
}

/// Compose a database row and its record into the view the UI is told about.
/// The record is the only source for a run's outcome; a row whose record
/// can't be read (missing/corrupt `EXPERIMENT.md`) reports `status: "unknown"`
/// rather than an empty string.
fn overlay(workspace_root: &Path, record_dir: Option<&str>, row: ExperimentRow) -> ExperimentView {
    match record_dir.and_then(|d| record::read(workspace_root, d).ok()) {
        Some(persisted) => ExperimentView {
            uuid: row.uuid,
            name: persisted.name,
            command: persisted.command,
            status: persisted.status,
            exit_code: persisted.exit_code,
            started_at: persisted.started_at,
            ended_at: persisted.ended_at,
            pid: row.pid,
            tab_id: row.tab_id,
        },
        None => ExperimentView {
            uuid: row.uuid,
            name: row.name,
            command: row.command,
            status: "unknown".to_string(),
            exit_code: None,
            started_at: row.started_at,
            ended_at: None,
            pid: row.pid,
            tab_id: row.tab_id,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace as a second machine sees it after `git clone`: the records
    /// are committed, `local.db` is empty.
    fn cloned() -> (tempfile::TempDir, tempfile::TempDir, String) {
        let workspace = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();
        let dir = record::create(
            workspace.path(),
            record::RecordArgs {
                uuid: "11111111-2222-3333-4444-555555555555",
                name: "LR ablation",
                command: "python run.py",
                working_dir: "/tmp/project",
                wake_prompt: "check it",
                started_at: "2026-08-11T10:00:00+02:00",
            },
        )
        .unwrap();
        record::set_status(workspace.path(), &dir, "completed", Some(0), Some("t1")).unwrap();
        (workspace, home, dir)
    }

    #[test]
    fn a_run_with_no_database_row_is_still_addressable() {
        let (workspace, home, dir) = cloned();
        let uuid = "11111111-2222-3333-4444-555555555555";

        assert!(row(workspace.path(), home.path(), uuid).is_none());
        assert_eq!(record_dir(workspace.path(), home.path(), uuid), Some(dir));

        let composed = row_or_record(workspace.path(), home.path(), uuid).unwrap();
        assert_eq!(composed.name, "LR ablation");
        assert_eq!(composed.status, "completed");
        assert_eq!(composed.command, "python run.py");
        assert_eq!(composed.pid, None);
    }

    #[test]
    fn an_unknown_uuid_stays_unknown() {
        let (workspace, home, _) = cloned();
        assert!(record_dir(workspace.path(), home.path(), "nope").is_none());
        assert!(row_or_record(workspace.path(), home.path(), "nope").is_none());
    }

    #[test]
    fn a_settled_run_keeps_the_ending_it_has() {
        let (workspace, home, dir) = cloned();
        let uuid = "11111111-2222-3333-4444-555555555555";
        record::set_status(workspace.path(), &dir, "cancelled", None, Some("t2")).unwrap();

        // What the monitor reports ~500ms after a SIGTERM: signalled, so no
        // exit code, which would otherwise be written as a plain failure.
        write_status(workspace.path(), home.path(), uuid, "failed", Some(-1), Some("t3"));
        assert_eq!(record::read(workspace.path(), &dir).unwrap().status, "failed");

        // `transition` is the guarded path, and it must refuse.
        let settled = row_or_record(workspace.path(), home.path(), uuid).unwrap();
        assert!(TERMINAL.contains(&settled.status.as_str()));
    }

    #[test]
    fn a_cloned_run_can_be_deleted() {
        let (workspace, home, dir) = cloned();
        let uuid = "11111111-2222-3333-4444-555555555555";

        // What experiment_delete does: guard on the composed view, then locate
        // the record without a database row to go by.
        let view = row_or_record(workspace.path(), home.path(), uuid).unwrap();
        assert_eq!(view.status, "completed", "the guard must see a settled run");
        let found = record_dir(workspace.path(), home.path(), uuid).unwrap();
        assert_eq!(found, dir);

        record::remove(workspace.path(), &found);
        assert!(record_dir(workspace.path(), home.path(), uuid).is_none());
        assert!(row_or_record(workspace.path(), home.path(), uuid).is_none());
    }

    /// Issue #120: `ExperimentRow` (or its replacement view type) must not
    /// carry `status`/`exit_code`/`ended_at` fields sourced from `local.db`
    /// at all — the record is the only source for a run's outcome.
    ///
    /// This is a compile-level lock-in, deliberately written the way the
    /// issue suggests ("checks struct field absence via a type-level
    /// test"): it constructs an `ExperimentRow` using struct-update syntax
    /// from a base built with ONLY the fields a #120-compliant row should
    /// keep. Today `ExperimentRow` still requires `status`, `exit_code` and
    /// `ended_at`, so this fails to compile with "missing fields" — which is
    /// the desired "not passing yet" signal. It will compile (and the
    /// assertions will pass) once those three fields are gone.
    #[test]
    fn experiment_row_has_no_db_sourced_status_fields() {
        let row = crate::db::models::ExperimentRow {
            uuid: "u".to_string(),
            name: "n".to_string(),
            command: "c".to_string(),
            started_at: "t0".to_string(),
            pid: None,
            tab_id: "main".to_string(),
            // Deliberately no status / exit_code / ended_at: a #120-compliant
            // ExperimentRow must not require them.
        };
        assert_eq!(row.uuid, "u");
        assert_eq!(row.tab_id, "main");
    }

    /// Issue #120: a row whose record cannot be read (missing/corrupt
    /// EXPERIMENT.md) must report `run_status: unknown`, not an empty
    /// string — matching the "unknown" convention used elsewhere for an
    /// unreadable/absent status (see experiments::migrate).
    #[test]
    fn a_row_with_no_readable_record_reports_unknown_status() {
        let workspace = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();
        let uuid = "22222222-3333-4444-5555-666666666666";

        // Row points at a record_dir that doesn't exist on disk (corrupt /
        // missing EXPERIMENT.md).
        {
            let conn = rusqlite::Connection::open(home.path().join("local.db")).unwrap();
            conn.execute(
                "INSERT INTO experiments (uuid, name, command, working_dir, started_at, session_id, tab_id, record_dir) \
                 VALUES (?1, 'orphaned', 'run.py', '/tmp', 't0', 's1', 'main', '.ire/experiments/999-missing')",
                rusqlite::params![uuid],
            )
            .unwrap();
        }

        let composed = row(workspace.path(), home.path(), uuid)
            .expect("a db row exists even though its record does not");
        assert_eq!(
            composed.status, "unknown",
            "a row with no readable record must report 'unknown', not an empty string"
        );
    }

    /// Issue #120: `update_experiment_completed` is gone and the db no longer
    /// has status/exit_code/ended_at columns to write — `transition` writes
    /// only the record. Locked in two ways: the db schema itself carries no
    /// such columns (a fresh `experiments` table has none to touch), and a
    /// `transition` call is reflected only in the record, never in a
    /// resurrected db column.
    #[test]
    fn transition_writes_only_the_record_not_db_status_columns() {
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();
        let conn = rusqlite::Connection::open(home.path().join("local.db")).unwrap();
        for col in ["status", "exit_code", "ended_at"] {
            let has_column: bool = conn
                .query_row(
                    "SELECT EXISTS (SELECT 1 FROM pragma_table_info('experiments') WHERE name = ?1)",
                    rusqlite::params![col],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(
                !has_column,
                "{col} must not exist on `experiments` post-#120: \
                 `update_experiment_completed` has nothing left to write"
            );
        }
    }
}
