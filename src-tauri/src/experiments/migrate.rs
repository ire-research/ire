//! One-time, lazy upgrade of a workspace whose experiment state predates
//! `EXPERIMENT.md` owning it: every record written before this gets a
//! frontmatter block backfilled from `local.db` and the outgoing `ire.json`
//! entry, and `ire.json`'s `experiments` array is then removed for good.
//!
//! Runs on workspace open, mirroring how `workspace::init` backfills missing
//! directories. Everything here is best-effort: a workspace that cannot be
//! upgraded is still openable, and the pass is idempotent, so the next open
//! simply tries again.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;

use super::record::{self, Record};
use crate::db::models as db;
use crate::ire::store::atomic_write;

const DIR: &str = ".ire/experiments";

pub fn run(workspace_root: &Path, home_data_dir: &Path) {
    let legacy = legacy_status_by_uuid(workspace_root);
    let mut all_upgraded = true;
    for dir in record_dirs(workspace_root) {
        if let Err(e) = upgrade(workspace_root, home_data_dir, &dir, &legacy) {
            tracing::warn!(error = %e, dir = %dir, "experiment record upgrade failed");
            all_upgraded = false;
        }
    }
    // `ire.json` is the fallback the backfill reads statuses out of. Dropping it
    // while a record still needs it would strand that run on `unknown` for good.
    // The pass is idempotent, so leaving it costs only a retry next open.
    if !all_upgraded {
        tracing::warn!("keeping ire.json experiments: some records did not upgrade");
        return;
    }
    if let Err(e) = drop_ire_json_experiments(workspace_root) {
        tracing::warn!(error = %e, "dropping ire.json experiments failed");
    }
}

/// Give one record its frontmatter and point its database row at it. A record
/// that already has frontmatter only gets the row pointer refreshed, which is
/// what makes re-running harmless.
fn upgrade(
    workspace_root: &Path,
    home_data_dir: &Path,
    rel_dir: &str,
    legacy: &HashMap<String, LegacyStatus>,
) -> Result<()> {
    let content = fs::read_to_string(workspace_root.join(rel_dir).join("EXPERIMENT.md"))?;
    if record::has_frontmatter(&content) {
        // The uuid comes from the block, not from a `- **uuid**:` bullet: the
        // backfill below deletes those, and records created after this feature
        // never had one. Reading the bullet here would make the branch dead and
        // leave `record_dir` NULL forever, silently dropping every later status
        // write for this run.
        let uuid = record::read(workspace_root, rel_dir)?.uuid;
        if !uuid.is_empty() {
            db::set_experiment_record_dir(home_data_dir, &uuid, rel_dir)?;
        }
        return Ok(());
    }

    let Some(uuid) = legacy_field(&content, "uuid") else {
        // No uuid to key on: nothing can be said about how this run ended.
        return Ok(());
    };
    let row = db::get_experiment(home_data_dir, &uuid).ok().flatten();
    let fallback = legacy.get(&uuid);

    let record = Record {
        name: legacy_h1(&content).unwrap_or_else(|| {
            row.as_ref()
                .map(|r| r.name.clone())
                .unwrap_or_else(|| uuid.clone())
        }),
        command: String::new(), // read back from the body, never from the block
        goal: String::new(),    // likewise: it stays in the body's Goal & context
        status: row
            .as_ref()
            .map(|r| r.status.clone())
            .or_else(|| fallback.map(|f| f.status.clone()))
            // Predates both stores: say so rather than claiming it still runs.
            .unwrap_or_else(|| "unknown".to_string()),
        exit_code: row
            .as_ref()
            .and_then(|r| r.exit_code)
            .or_else(|| fallback.and_then(|f| f.exit_code)),
        started_at: legacy_field(&content, "started").unwrap_or_else(|| {
            row.as_ref()
                .map(|r| r.started_at.clone())
                .unwrap_or_default()
        }),
        ended_at: row
            .as_ref()
            .and_then(|r| r.ended_at.clone())
            .or_else(|| fallback.and_then(|f| f.ended_at.clone())),
        uuid: uuid.clone(),
    };
    let working_dir = legacy_field(&content, "working dir").unwrap_or_default();

    record::backfill(
        workspace_root,
        rel_dir,
        &record,
        &working_dir,
        &strip_legacy_header(&content),
    )?;
    db::set_experiment_record_dir(home_data_dir, &uuid, rel_dir)?;
    Ok(())
}

struct LegacyStatus {
    status: String,
    exit_code: Option<i64>,
    ended_at: Option<String>,
}

/// The `experiments` array of `ire.json`, read as raw JSON because the typed
/// schema no longer has the field.
fn legacy_status_by_uuid(workspace_root: &Path) -> HashMap<String, LegacyStatus> {
    let Ok(raw) = fs::read_to_string(workspace_root.join(".ire/ire.json")) else {
        return HashMap::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return HashMap::new();
    };
    let Some(entries) = value.get("experiments").and_then(|e| e.as_array()) else {
        return HashMap::new();
    };
    entries
        .iter()
        .filter_map(|e| {
            let uuid = e.get("uuid")?.as_str()?.to_string();
            Some((
                uuid,
                LegacyStatus {
                    status: e.get("status")?.as_str()?.to_string(),
                    exit_code: e.get("exit_code").and_then(serde_json::Value::as_i64),
                    ended_at: e
                        .get("ended_at")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                },
            ))
        })
        .collect()
}

/// Rewrite `ire.json` without its `experiments` key, preserving everything
/// else byte-for-byte where serde round-trips it. A file that never had the
/// key is left alone, so this does not churn already-upgraded workspaces.
fn drop_ire_json_experiments(workspace_root: &Path) -> Result<()> {
    let path = workspace_root.join(".ire/ire.json");
    let Ok(raw) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(());
    };
    let Some(object) = value.as_object_mut() else {
        return Ok(());
    };
    if object.remove("experiments").is_none() {
        return Ok(());
    }
    atomic_write(&path, &(serde_json::to_string_pretty(&value)? + "\n"))
}

fn record_dirs(workspace_root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(workspace_root.join(DIR)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().to_str().map(|n| format!("{DIR}/{n}")))
        .collect()
}

/// A `- **key**: value` line from the pre-frontmatter header, with backticks
/// stripped off the values that carried them.
fn legacy_field(content: &str, key: &str) -> Option<String> {
    let prefix = format!("- **{key}**:");
    content
        .lines()
        .find_map(|l| l.trim().strip_prefix(&prefix).map(str::trim))
        .map(|v| v.trim_matches('`').to_string())
        .filter(|v| !v.is_empty())
}

fn legacy_h1(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Drop the header the frontmatter now replaces: the H1 stays (it keeps the
/// file readable where YAML isn't rendered), the three restated fields go.
///
/// The blank line the removed block leaves behind is collapsed right there,
/// rather than by rewriting every double blank line in the document — the body
/// can contain fenced code whose spacing is not ours to normalize.
fn strip_legacy_header(content: &str) -> String {
    const RESTATED: [&str; 3] = ["uuid", "started", "working dir"];
    let mut out = String::new();
    let mut just_removed = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if RESTATED
            .iter()
            .any(|k| trimmed.starts_with(&format!("- **{k}**:")))
        {
            just_removed = true;
            continue;
        }
        // Swallow exactly the one blank line that followed the removed block.
        if just_removed && trimmed.is_empty() {
            just_removed = false;
            continue;
        }
        just_removed = false;
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY: &str = "# LR ablation\n\n\
        - **uuid**: `11111111-2222-3333-4444-555555555555`\n\
        - **started**: 2026-08-11T10:00:00+02:00\n\
        - **working dir**: `/tmp/project`\n\n\
        ## Goal & context\n\n\
        Check whether lr=1e-4 beats the baseline.\n\n\
        ## Command\n\n\
        ```sh\npython run.py --lr 1e-4\n```\n";

    fn workspace(legacy_ire_json: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(DIR).join("001-lr-ablation");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("EXPERIMENT.md"), LEGACY).unwrap();
        fs::write(tmp.path().join(".ire/ire.json"), legacy_ire_json).unwrap();
        tmp
    }

    const IRE_JSON: &str = "{\"notes\":\"keep me\",\"focus\":{\"research_question\":\"rq\",\
        \"this_week\":\"tw\"},\"ideas\":[],\"experiments\":[{\"uuid\":\
        \"11111111-2222-3333-4444-555555555555\",\"name\":\"LR ablation\",\"command\":\"x\",\
        \"status\":\"completed\",\"started_at\":\"2026-08-11T10:00:00+02:00\",\
        \"ended_at\":\"2026-08-11T11:00:00+02:00\",\"exit_code\":0}]}\n";

    #[test]
    fn backfills_frontmatter_from_the_outgoing_ire_json_entry() {
        let tmp = workspace(IRE_JSON);
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();

        run(tmp.path(), home.path());

        let rec = record::read(tmp.path(), ".ire/experiments/001-lr-ablation").unwrap();
        assert_eq!(rec.uuid, "11111111-2222-3333-4444-555555555555");
        assert_eq!(rec.name, "LR ablation");
        assert_eq!(rec.status, "completed");
        assert_eq!(rec.exit_code, Some(0));
        assert_eq!(rec.started_at, "2026-08-11T10:00:00+02:00");
        assert_eq!(rec.ended_at.as_deref(), Some("2026-08-11T11:00:00+02:00"));
        // The command still comes from the body, which the upgrade preserved.
        assert_eq!(rec.command, "python run.py --lr 1e-4");
    }

    #[test]
    fn upgrade_keeps_the_body_and_drops_only_the_restated_header() {
        let tmp = workspace(IRE_JSON);
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();

        run(tmp.path(), home.path());

        let md =
            fs::read_to_string(tmp.path().join(DIR).join("001-lr-ablation/EXPERIMENT.md")).unwrap();
        assert!(md.starts_with("---\ntype: Experiment\n"));
        assert!(md.contains("# LR ablation"));
        assert!(md.contains("Check whether lr=1e-4 beats the baseline."));
        assert!(!md.contains("- **uuid**"));
        assert!(!md.contains("- **working dir**"));
    }

    #[test]
    fn ire_json_loses_experiments_and_keeps_everything_else() {
        let tmp = workspace(IRE_JSON);
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();

        run(tmp.path(), home.path());

        let raw = fs::read_to_string(tmp.path().join(".ire/ire.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(value.get("experiments").is_none());
        assert_eq!(value["notes"], "keep me");
        assert_eq!(value["focus"]["research_question"], "rq");
    }

    #[test]
    fn rerunning_is_a_no_op() {
        let tmp = workspace(IRE_JSON);
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();

        run(tmp.path(), home.path());
        let after_first =
            fs::read_to_string(tmp.path().join(DIR).join("001-lr-ablation/EXPERIMENT.md")).unwrap();
        run(tmp.path(), home.path());
        let after_second =
            fs::read_to_string(tmp.path().join(DIR).join("001-lr-ablation/EXPERIMENT.md")).unwrap();
        assert_eq!(after_first, after_second);
    }

    #[test]
    fn a_record_with_no_status_anywhere_is_marked_unknown() {
        let tmp = workspace("{\"notes\":\"\",\"ideas\":[]}\n");
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();

        run(tmp.path(), home.path());

        let rec = record::read(tmp.path(), ".ire/experiments/001-lr-ablation").unwrap();
        assert_eq!(rec.status, "unknown");
        assert_eq!(rec.exit_code, None);
    }

    #[test]
    fn an_already_upgraded_record_still_gets_its_row_pointer() {
        let tmp = workspace(IRE_JSON);
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();
        let uuid = "11111111-2222-3333-4444-555555555555";
        crate::db::models::insert_experiment(
            home.path(), uuid, "LR ablation", "python run.py", "/tmp/project",
            "session", "main", "2026-08-11T10:00:00+02:00", "",
        )
        .unwrap();

        // First pass upgrades the file; simulate its row pointer never landing
        // (killed mid-migration, or local.db reset afterwards).
        run(tmp.path(), home.path());
        crate::db::models::set_experiment_record_dir(home.path(), uuid, "").unwrap();

        // The second pass must repair it, or every later status write is lost:
        // write_status gives up when record_dir is empty.
        run(tmp.path(), home.path());
        assert_eq!(
            crate::db::models::get_experiment_record_dir(home.path(), uuid).unwrap(),
            Some(".ire/experiments/001-lr-ablation".to_string())
        );
    }

    #[test]
    fn blank_lines_inside_a_code_fence_are_left_alone() {
        let body = concat!(
            "# T\n\n",
            "- **uuid**: `u`\n",
            "- **started**: t\n",
            "- **working dir**: `/w`\n\n",
            "## Command\n\n",
            "```sh\necho a\n\n\necho b\n```\n",
        );
        let out = strip_legacy_header(body);
        assert!(out.contains("echo a\n\n\necho b"), "code fence was reflowed: {out:?}");
        assert!(!out.contains("- **uuid**"));
        assert!(out.starts_with("# T\n\n## Command"), "{out:?}");
    }

    #[test]
    fn a_failed_record_upgrade_keeps_the_ire_json_fallback() {
        let tmp = workspace(IRE_JSON);
        let home = tempfile::tempdir().unwrap();
        crate::db::schema::run(home.path()).unwrap();

        // A record that cannot be read at all: its folder has no EXPERIMENT.md.
        fs::create_dir_all(tmp.path().join(DIR).join("002-unreadable")).unwrap();

        run(tmp.path(), home.path());

        let raw = fs::read_to_string(tmp.path().join(".ire/ire.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            value.get("experiments").is_some(),
            "the fallback was dropped while a record still needed it"
        );
    }
}
