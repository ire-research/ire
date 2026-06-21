pub mod runner;
pub mod wake;

use std::path::Path;

use crate::db::models::ExperimentRow;
use crate::ire::{IreExperiment, IreStore};

/// Mirror a DB experiment row into the git-tracked `ire.json` display record.
/// Best-effort: a failure is logged, not propagated.
pub fn sync_to_ire(workspace_root: &Path, row: &ExperimentRow) {
    let store = IreStore::new(workspace_root.to_path_buf());
    let exp = IreExperiment {
        uuid: row.uuid.clone(),
        name: row.name.clone(),
        command: row.command.clone(),
        status: row.status.clone(),
        started_at: row.started_at.clone(),
        ended_at: row.ended_at.clone(),
        exit_code: row.exit_code,
    };
    if let Err(e) = store.upsert_experiment(exp) {
        tracing::warn!(error = %e, uuid = %row.uuid, "sync experiment to ire.json failed");
    }
}

/// Remove an experiment from the `ire.json` display record. Best-effort.
pub fn remove_from_ire(workspace_root: &Path, uuid: &str) {
    let store = IreStore::new(workspace_root.to_path_buf());
    if let Err(e) = store.remove_experiment(uuid) {
        tracing::warn!(error = %e, uuid = %uuid, "remove experiment from ire.json failed");
    }
}
