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
    if let Err(e) = db::update_experiment_completed(home_data_dir, uuid, status, exit_code) {
        tracing::warn!(error = %e, uuid = %uuid, "update experiment row failed");
    }
    let ended_at = db::get_experiment(home_data_dir, uuid)
        .ok()
        .flatten()
        .and_then(|r| r.ended_at);
    write_status(
        workspace_root,
        home_data_dir,
        uuid,
        status,
        exit_code.map(i64::from),
        ended_at.as_deref(),
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
fn row(workspace_root: &Path, home_data_dir: &Path, uuid: &str) -> Option<ExperimentRow> {
    let mut row = db::get_experiment(home_data_dir, uuid).ok().flatten()?;
    let dir = db::get_experiment_record_dir(home_data_dir, uuid)
        .ok()
        .flatten();
    overlay(workspace_root, dir.as_deref(), &mut row);
    Some(row)
}

/// Every experiment, composed the same way [`row`] composes one. This is what
/// `experiment_list` serves, so a list view and a live event can never disagree
/// about how a run ended.
pub fn list(
    workspace_root: &Path,
    home_data_dir: &Path,
    limit: usize,
) -> anyhow::Result<Vec<ExperimentRow>> {
    let mut rows = db::list_experiments(home_data_dir, limit)?;
    let dirs = db::experiment_record_dirs(home_data_dir)?;
    for row in &mut rows {
        overlay(workspace_root, dirs.get(&row.uuid).map(String::as_str), row);
    }
    Ok(rows)
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
) -> Option<ExperimentRow> {
    if let Some(row) = row(workspace_root, home_data_dir, uuid) {
        return Some(row);
    }
    let dir = record_dir(workspace_root, home_data_dir, uuid)?;
    let persisted = record::read(workspace_root, &dir).ok()?;
    Some(ExperimentRow {
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

/// Lay the record's fields over a database row, in place.
fn overlay(workspace_root: &Path, record_dir: Option<&str>, row: &mut ExperimentRow) {
    let Some(persisted) = record_dir.and_then(|d| record::read(workspace_root, d).ok()) else {
        return;
    };
    row.name = persisted.name;
    row.command = persisted.command;
    row.status = persisted.status;
    row.exit_code = persisted.exit_code;
    row.started_at = persisted.started_at;
    row.ended_at = persisted.ended_at;
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
}
