//! The git-tracked `.ire/experiments/<NNN>-<slug>/` folder created when an
//! experiment starts. `EXPERIMENT.md` inside it is the single source of truth
//! for what ran, why, and how it ended: an OKF-shaped YAML frontmatter block
//! owned exclusively by the runner, over a body owned by whoever writes to it.
//! Status transitions rewrite the block and nothing else, so notes an agent or
//! a person appends below survive every rewrite. The folder is also the home
//! for the experiment's own artifacts; raw logs stay in
//! `.ire/cache/experiments/<uuid>/`.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};

use okf::yaml::Value;
use okf::Frontmatter;

use crate::ire::frontmatter;
use crate::ire::store::atomic_write;

/// Serializes prefix allocation so two experiments starting at once can't claim
/// the same number. A workspace is single-instance (see `workspace::lock`), so
/// an in-process lock covers every writer.
static ALLOC_LOCK: Mutex<()> = Mutex::new(());

/// Serializes `EXPERIMENT.md` read-modify-write cycles. `atomic_write` makes a
/// single write atomic, not the read-mutate-write around it: the monitor thread
/// finishing a run and a UI rename can otherwise both read `run_status:
/// running`, and whichever writes second silently drops the other's change —
/// leaving a finished run displayed as running forever, since the monitor has
/// already stopped. A workspace is single-instance, so an in-process lock is enough.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

const DIR: &str = ".ire/experiments";

pub struct RecordArgs<'a> {
    pub uuid: &'a str,
    pub name: &'a str,
    pub command: &'a str,
    pub working_dir: &'a str,
    pub wake_prompt: &'a str,
    pub started_at: &'a str,
}

/// Create `.ire/experiments/<NNN>-<slug>/EXPERIMENT.md`, returning the folder
/// path relative to the workspace root. Creates `.ire/experiments/` on demand,
/// so workspaces initialized before this existed pick it up on their next run.
pub fn create(workspace_root: &Path, args: RecordArgs<'_>) -> Result<String> {
    let root = workspace_root.join(DIR);
    let _guard = ALLOC_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;

    let name = format!("{:03}-{}", next_prefix(&root), slugify(args.name));
    let dir = root.join(&name);
    fs::create_dir(&dir).with_context(|| format!("create {}", dir.display()))?;
    atomic_write(&dir.join(FILE), &render(&args))?;
    Ok(format!("{DIR}/{name}"))
}

/// Undo [`create`] when the experiment never started. Best-effort: a failure is
/// logged, not propagated — the spawn error is what the caller reports.
pub fn remove(workspace_root: &Path, rel_dir: &str) {
    let dir = workspace_root.join(rel_dir);
    if let Err(e) = fs::remove_dir_all(&dir) {
        tracing::warn!(error = %e, dir = %dir.display(), "remove experiment record failed");
    }
}

/// The numeric prefix a record folder was allocated, for ordering. A folder
/// that doesn't carry one sorts oldest.
fn prefix_of(name: &str) -> u32 {
    name.split_once('-')
        .and_then(|(prefix, _)| prefix.parse().ok())
        .unwrap_or(0)
}

/// One past the highest `NNN-` prefix present. Gaps are left alone: numbering
/// only moves forward, so deleting a folder never reissues its number.
fn next_prefix(root: &Path) -> u32 {
    let Ok(entries) = fs::read_dir(root) else {
        return 1;
    };
    let highest = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name();
            let (prefix, _) = name.to_str()?.split_once('-')?;
            prefix.parse::<u32>().ok()
        })
        .max();
    highest.unwrap_or(0) + 1
}

/// Filesystem-safe title slug: ASCII alphanumerics lowercased, every other run
/// of characters collapsed to a single `-`.
fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let slug: String = out.trim_matches('-').chars().take(60).collect();
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "experiment".to_string()
    } else {
        slug.to_string()
    }
}

/// What `EXPERIMENT.md`'s frontmatter says, as the runner last wrote it.
#[derive(Debug, Clone)]
pub struct Record {
    pub uuid: String,
    pub name: String,
    pub command: String,
    /// Goal and context, from the body's `## Goal & context` section. This is
    /// the wake-up prompt: it is not stored anywhere else.
    pub goal: String,
    pub status: String,
    pub exit_code: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
}

/// Read one experiment's record back from disk.
pub fn read(workspace_root: &Path, rel_dir: &str) -> Result<Record> {
    let path = workspace_root.join(rel_dir).join(FILE);
    let content = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    parse_record(&content).ok_or_else(|| anyhow!("no frontmatter in {}", path.display()))
}

/// Every experiment record in the workspace, newest first. Folders without a
/// readable record are skipped: this feeds the UI, not a consistency check.
pub fn list(workspace_root: &Path) -> Vec<(String, Record)> {
    let root = workspace_root.join(DIR);
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut dirs: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    // Prefixes are allocated in start order, so prefix order is time order.
    // Sorted numerically, not as strings: past 999 the width changes and
    // "1000" sorts before "999".
    dirs.sort_unstable_by(|a, b| prefix_of(b).cmp(&prefix_of(a)).then_with(|| b.cmp(a)));
    dirs.into_iter()
        .filter_map(|name| {
            let rel = format!("{DIR}/{name}");
            read(workspace_root, &rel).ok().map(|r| (rel, r))
        })
        .collect()
}

/// Rewrite the runner-owned frontmatter for a status transition and return the
/// record as it was actually persisted. The body is untouched.
pub fn set_status(
    workspace_root: &Path,
    rel_dir: &str,
    status: &str,
    exit_code: Option<i64>,
    ended_at: Option<&str>,
) -> Result<Record> {
    rewrite(workspace_root, rel_dir, |r| {
        r.status = status.to_string();
        r.exit_code = exit_code;
        r.ended_at = ended_at.map(str::to_string);
    })
}

/// Rewrite the frontmatter `title`. The `# {name}` H1 in the body is left as
/// written: the body belongs to whoever edits it, not to the runner.
pub fn set_title(workspace_root: &Path, rel_dir: &str, name: &str) -> Result<Record> {
    rewrite(workspace_root, rel_dir, |r| r.name = name.to_string())
}

fn rewrite(
    workspace_root: &Path,
    rel_dir: &str,
    mutate: impl FnOnce(&mut Record),
) -> Result<Record> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = workspace_root.join(rel_dir).join(FILE);
    let content = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut fm = frontmatter::parse(&content)
        .0
        .ok_or_else(|| anyhow!("no frontmatter in {}", path.display()))?;
    let mut record =
        parse_record(&content).ok_or_else(|| anyhow!("no frontmatter in {}", path.display()))?;
    mutate(&mut record);
    // Set only the fields the runner owns, on the block as it exists. Rebuilding
    // it would drop any key someone added (OKF §4.1: preserve unknown keys).
    apply(&mut fm, &record);
    atomic_write(&path, &frontmatter::replace(&content, &fm))?;
    // Report what is on disk, never what we meant to put there.
    read(workspace_root, rel_dir)
}

const FILE: &str = "EXPERIMENT.md";

/// Write the runner-owned fields onto an existing block, leaving every other
/// key — and the order they sit in — alone.
fn apply(fm: &mut Frontmatter, record: &Record) {
    fm.set("title", Value::String(record.name.clone()));
    fm.set("run_status", Value::String(record.status.clone()));
    fm.set("exit_code", record.exit_code.map_or(Value::Null, Value::Int));
    fm.set(
        "ended_at",
        record.ended_at.clone().map_or(Value::Null, Value::String),
    );
}

/// The block a brand-new record starts with, in a fixed field order so a status
/// transition shows up in `git diff` as the lines that actually changed.
///
/// The run state is `run_status`, not `status`: OKF §5.4 gives `status` the
/// lifecycle meaning `draft | stable | deprecated`, and once claims and
/// resources share this bundle a single key cannot mean both.
fn frontmatter_for(record: &Record, working_dir: &str) -> Frontmatter {
    let mut fm = Frontmatter::new();
    fm.set("type", Value::String("Experiment".into()));
    fm.set("title", Value::String(record.name.clone()));
    fm.set("uuid", Value::String(record.uuid.clone()));
    fm.set("started_at", Value::String(record.started_at.clone()));
    fm.set("working_dir", Value::String(working_dir.to_string()));
    fm.set("run_status", Value::String(record.status.clone()));
    fm.set("exit_code", record.exit_code.map_or(Value::Null, Value::Int));
    fm.set(
        "ended_at",
        record.ended_at.clone().map_or(Value::Null, Value::String),
    );
    fm
}

/// Write a record that has no frontmatter yet — the lazy upgrade of a
/// pre-frontmatter `EXPERIMENT.md`. `body` becomes everything below the block.
pub fn backfill(
    workspace_root: &Path,
    rel_dir: &str,
    record: &Record,
    working_dir: &str,
    body: &str,
) -> Result<()> {
    let mut content = frontmatter::render(&frontmatter_for(record, working_dir));
    content.push_str(body);
    atomic_write(&workspace_root.join(rel_dir).join(FILE), &content)
}

/// Whether this file already carries a frontmatter block.
pub fn has_frontmatter(content: &str) -> bool {
    frontmatter::parse(content).0.is_some()
}

fn parse_record(content: &str) -> Option<Record> {
    let (fm, _) = frontmatter::parse(content);
    let fm = fm?;
    let get = |k: &str| frontmatter::field(&fm, k).unwrap_or_default();
    Some(Record {
        uuid: get("uuid"),
        name: get("title"),
        command: body_command(content),
        goal: body_section(content, "## Goal & context"),
        status: get("run_status"),
        exit_code: fm.get("exit_code").and_then(okf::yaml::Value::as_int),
        started_at: get("started_at"),
        ended_at: frontmatter::field(&fm, "ended_at").filter(|v| !v.is_empty()),
    })
}

/// The command back out of the body's `## Command` fence. It is written once
/// and never rewritten, so reading it is what keeps the file — not the
/// database — the record the UI is built from.
///
/// Scans the body only, matches the heading exactly, and closes on a fence of
/// the same backtick run it opened with. A record whose body has been edited
/// past recognition yields an empty command rather than a swallowed document.
/// The prose under a `## ` heading, up to the next heading of any level.
fn body_section(content: &str, heading: &str) -> String {
    let body = frontmatter::parse(content).1;
    body.lines()
        .skip_while(|l| l.trim_end() != heading)
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn body_command(content: &str) -> String {
    let body = frontmatter::parse(content).1;
    let mut lines = body
        .lines()
        .skip_while(|l| l.trim_end() != "## Command")
        .skip(1)
        .skip_while(|l| l.trim().is_empty());

    let Some(open) = lines.next() else {
        return String::new();
    };
    let fence: String = open.trim_start().chars().take_while(|c| *c == '`').collect();
    if fence.len() < 3 {
        return String::new();
    }
    let mut out: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim_end() == fence {
            return out.join("\n");
        }
        out.push(line);
    }
    // Unterminated fence: the command is not recoverable, and returning the
    // rest of the document as one would be worse than returning nothing.
    String::new()
}

fn render(args: &RecordArgs<'_>) -> String {
    let fence = fence_for(args.command);
    let record = Record {
        uuid: args.uuid.to_string(),
        name: args.name.trim().to_string(),
        command: args.command.to_string(),
        goal: args.wake_prompt.trim().to_string(),
        status: "running".to_string(),
        exit_code: None,
        started_at: args.started_at.to_string(),
        ended_at: None,
    };
    format!(
        "{fm}
\
         # {name}
\n\
         ## Goal & context\n\n\
         {wake_prompt}
\n\
         ## Command\n\n\
         {fence}sh\n{command}
{fence}
\n\
         ---\n\n\
         Artifacts belonging to this experiment — scripts, result files, notes — go in\n\
         this folder. Raw stdout/stderr stay in `.ire/cache/experiments/{uuid}/`.\n",
        fm = frontmatter::render(&frontmatter_for(&record, args.working_dir)),
        name = record.name,
        uuid = args.uuid,
        wake_prompt = args.wake_prompt.trim(),
        command = args.command,
    )
}

/// A fence long enough that a command containing backticks can't break out.
fn fence_for(command: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in command.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat(longest.max(2) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args<'a>(name: &'a str, command: &'a str) -> RecordArgs<'a> {
        RecordArgs {
            uuid: "11111111-2222-3333-4444-555555555555",
            name,
            command,
            working_dir: "/tmp/project",
            wake_prompt: "Check whether lr=1e-4 beats the baseline.",
            started_at: "2026-08-11T10:00:00+02:00",
        }
    }

    #[test]
    fn first_experiment_allocates_001() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("LR ablation", "echo hi")).unwrap();
        assert_eq!(dir, ".ire/experiments/001-lr-ablation");
        assert!(tmp.path().join(&dir).join("EXPERIMENT.md").exists());
    }

    #[test]
    fn allocation_continues_past_existing_dirs_and_gaps() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(DIR);
        for existing in ["001-first", "004-fourth", "not-numbered"] {
            fs::create_dir_all(root.join(existing)).unwrap();
        }
        // Highest is 004 despite the 002/003 gap, and a loose file is ignored.
        fs::write(root.join("009-a-file-not-a-dir"), "").unwrap();
        let dir = create(tmp.path(), args("fifth", "echo hi")).unwrap();
        assert_eq!(dir, ".ire/experiments/005-fifth");
    }

    #[test]
    fn missing_experiments_dir_is_created_for_old_workspaces() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!tmp.path().join(DIR).exists());
        create(tmp.path(), args("first", "echo hi")).unwrap();
        assert!(tmp.path().join(DIR).join("001-first").is_dir());
    }

    #[test]
    fn slugify_normalizes_titles() {
        assert_eq!(slugify("LR Ablation"), "lr-ablation");
        assert_eq!(slugify("  spaced  out  "), "spaced-out");
        assert_eq!(slugify("a/b\\c:d*e?"), "a-b-c-d-e");
        assert_eq!(slugify("__leading and trailing__"), "leading-and-trailing");
        assert_eq!(slugify("!!!"), "experiment");
        assert_eq!(slugify(""), "experiment");
        assert_eq!(slugify("..hidden"), "hidden");
        assert_eq!(slugify(&"x".repeat(100)).len(), 60);
        // A truncation landing on a separator doesn't leave a trailing dash.
        assert!(!slugify(&format!("{} tail", "x".repeat(59))).ends_with('-'));
    }

    #[test]
    fn record_carries_goal_command_and_shared_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("LR ablation", "python run.py --lr 1e-4")).unwrap();
        let md = fs::read_to_string(tmp.path().join(dir).join("EXPERIMENT.md")).unwrap();

        assert!(
            md.starts_with(
                "---\n\
             type: Experiment\n\
             title: LR ablation\n\
             uuid: 11111111-2222-3333-4444-555555555555\n\
             started_at: \"2026-08-11T10:00:00+02:00\"\n\
             working_dir: /tmp/project\n\
             run_status: running\n\
             exit_code: null\n\
             ended_at: null\n\
             ---\n\n\
             # LR ablation\n"
            ),
            "{md}"
        );
        assert!(md.contains("Check whether lr=1e-4 beats the baseline."));
        assert!(md.contains("```sh\npython run.py --lr 1e-4\n```"));
    }

    #[test]
    fn command_containing_backticks_cannot_break_the_fence() {
        let tmp = tempfile::tempdir().unwrap();
        let command = "echo ```nested``` && echo $(date)";
        let dir = create(tmp.path(), args("fences", command)).unwrap();
        let md = fs::read_to_string(tmp.path().join(dir).join("EXPERIMENT.md")).unwrap();
        assert!(md.contains(&format!("````sh\n{command}
````")));
    }

    #[test]
    fn concurrent_starts_get_distinct_numbers() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let dirs: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..8)
                .map(|i| {
                    let root = root.clone();
                    s.spawn(move || {
                        let name = format!("run {i}");
                        create(&root, args(&name, "echo hi")).unwrap()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let mut prefixes: Vec<&str> = dirs
            .iter()
            .map(|d| &d[".ire/experiments/".len()..][..3])
            .collect();
        prefixes.sort_unstable();
        prefixes.dedup();
        assert_eq!(prefixes.len(), 8, "duplicate prefix allocated: {dirs:?}");
        assert_eq!(
            prefixes,
            ["001", "002", "003", "004", "005", "006", "007", "008"]
        );
    }

    #[test]
    fn remove_deletes_the_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("doomed", "echo hi")).unwrap();
        assert!(tmp.path().join(&dir).exists());
        remove(tmp.path(), &dir);
        assert!(!tmp.path().join(&dir).exists());
        // The parent survives so the next allocation still sees history.
        assert!(tmp.path().join(DIR).is_dir());
    }

    #[test]
    fn a_status_transition_rewrites_only_the_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("LR ablation", "python run.py")).unwrap();
        let path = tmp.path().join(&dir).join(FILE);

        // What an agent or a person adds below the block must survive.
        let mut appended = fs::read_to_string(&path).unwrap();
        appended.push_str("\n## Findings\n\nlr=1e-4 wins by 0.4.\n");
        fs::write(&path, &appended).unwrap();
        let body_before = frontmatter::parse(&appended).1.to_string();

        let record = set_status(
            tmp.path(),
            &dir,
            "completed",
            Some(0),
            Some("2026-08-11T11:00:00+02:00"),
        )
        .unwrap();

        assert_eq!(record.status, "completed");
        assert_eq!(record.exit_code, Some(0));
        assert_eq!(
            record.ended_at.as_deref(),
            Some("2026-08-11T11:00:00+02:00")
        );

        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(frontmatter::parse(&after).1, body_before);
        assert!(after.contains("lr=1e-4 wins by 0.4."));
        assert!(after.contains("run_status: completed"));
        assert!(!after.contains("run_status: running"));
    }

    #[test]
    fn the_returned_record_is_what_landed_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("LR ablation", "python run.py")).unwrap();
        let written = set_status(
            tmp.path(),
            &dir,
            "failed",
            Some(2),
            Some("2026-08-11T12:00:00Z"),
        )
        .unwrap();
        let reread = read(tmp.path(), &dir).unwrap();
        assert_eq!(written.status, reread.status);
        assert_eq!(written.exit_code, reread.exit_code);
        assert_eq!(written.ended_at, reread.ended_at);
    }

    #[test]
    fn a_title_needing_quotes_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        // A colon would otherwise open a nested mapping; 1e-4 would parse as a float.
        let dir = create(tmp.path(), args("ablation: lr 1e-4", "echo hi")).unwrap();
        assert_eq!(read(tmp.path(), &dir).unwrap().name, "ablation: lr 1e-4");

        let renamed = set_title(tmp.path(), &dir, "\"quoted\" title").unwrap();
        assert_eq!(renamed.name, "\"quoted\" title");
        assert_eq!(read(tmp.path(), &dir).unwrap().name, "\"quoted\" title");
    }

    #[test]
    fn list_returns_records_newest_first() {
        let tmp = tempfile::tempdir().unwrap();
        create(tmp.path(), args("first", "echo 1")).unwrap();
        create(tmp.path(), args("second", "echo 2")).unwrap();
        let names: Vec<String> = list(tmp.path()).into_iter().map(|(_, r)| r.name).collect();
        assert_eq!(names, ["second", "first"]);
    }

    #[test]
    fn the_command_is_read_back_out_of_the_body() {
        let tmp = tempfile::tempdir().unwrap();
        let command = "echo ```nested``` && echo $(date)";
        let dir = create(tmp.path(), args("fences", command)).unwrap();
        assert_eq!(read(tmp.path(), &dir).unwrap().command, command);
    }

    #[test]
    fn remove_takes_the_artifacts_with_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("doomed with results", "echo hi")).unwrap();
        fs::write(tmp.path().join(&dir).join("results.csv"), "loss\n1.8\n").unwrap();

        remove(tmp.path(), &dir);

        assert!(!tmp.path().join(&dir).exists());
        // Gone from the record list, so hydrate can't resurrect it.
        assert!(list(tmp.path()).is_empty());
    }

    #[test]
    fn keys_a_person_added_survive_a_transition() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("LR ablation", "python run.py")).unwrap();
        let path = tmp.path().join(&dir).join(FILE);

        // OKF §4.1: consumers preserve unknown keys when round-tripping.
        let content = fs::read_to_string(&path).unwrap();
        let edited = content.replace(
            "run_status: running",
            "tags:\n  - ablation\ndescription: sweeping lr\nrun_status: running",
        );
        fs::write(&path, &edited).unwrap();

        set_status(tmp.path(), &dir, "completed", Some(0), Some("2026-08-11T11:00:00+02:00"))
            .unwrap();

        let after = fs::read_to_string(&path).unwrap();
        let (fm, _) = frontmatter::parse(&after);
        let fm = fm.unwrap();
        assert_eq!(fm.tags(), ["ablation"]);
        assert_eq!(fm.description().as_deref(), Some("sweeping lr"));
        assert_eq!(frontmatter::field(&fm, "run_status").as_deref(), Some("completed"));
    }

    #[test]
    fn a_rename_and_a_completion_cannot_lose_each_other() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("before", "echo hi")).unwrap();
        let root = tmp.path().to_path_buf();

        // The real race: the monitor thread finishing while the user renames.
        std::thread::scope(|s| {
            let (r1, d1) = (root.clone(), dir.clone());
            s.spawn(move || set_status(&r1, &d1, "completed", Some(0), Some("t1")).unwrap());
            let (r2, d2) = (root.clone(), dir.clone());
            s.spawn(move || set_title(&r2, &d2, "after").unwrap());
        });

        let rec = read(tmp.path(), &dir).unwrap();
        assert_eq!(rec.name, "after", "the rename was lost");
        assert_eq!(rec.status, "completed", "the completion was lost");
    }

    #[test]
    fn an_edited_body_cannot_swallow_the_document_as_a_command() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("edited", "python run.py")).unwrap();
        let path = tmp.path().join(&dir).join(FILE);

        // A user normalizing the fence language used to make `command` the
        // entire rest of the file.
        let content = fs::read_to_string(&path).unwrap();
        fs::write(&path, content.replace("```sh", "```bash")).unwrap();
        assert_eq!(read(tmp.path(), &dir).unwrap().command, "python run.py");

        // An unterminated fence yields nothing, not the remainder.
        let content = fs::read_to_string(&path).unwrap();
        fs::write(&path, content.replacen("```\n", "\n", 2)).unwrap();
        assert_eq!(read(tmp.path(), &dir).unwrap().command, "");
    }

    #[test]
    fn a_wake_prompt_mentioning_the_heading_does_not_win() {
        let tmp = tempfile::tempdir().unwrap();
        let mut a = args("tricky", "python run.py");
        a.wake_prompt = "Read the ## Command section when done.";
        let dir = create(tmp.path(), a).unwrap();
        assert_eq!(read(tmp.path(), &dir).unwrap().command, "python run.py");
    }

    #[test]
    fn the_goal_is_read_back_out_of_the_body() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("LR ablation", "python run.py")).unwrap();
        assert_eq!(
            read(tmp.path(), &dir).unwrap().goal,
            "Check whether lr=1e-4 beats the baseline."
        );
    }

    #[test]
    fn an_edited_goal_is_what_the_wake_up_gets() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("LR ablation", "python run.py")).unwrap();
        let path = tmp.path().join(&dir).join(FILE);

        // The body belongs to whoever edits it, so a revised goal is deliberate.
        let content = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            content.replace(
                "Check whether lr=1e-4 beats the baseline.",
                "Revised: compare against the 1e-3 run too.\n\nSecond paragraph.",
            ),
        )
        .unwrap();

        let rec = read(tmp.path(), &dir).unwrap();
        assert_eq!(
            rec.goal,
            "Revised: compare against the 1e-3 run too.\n\nSecond paragraph."
        );
        // A status transition must not disturb it.
        set_status(tmp.path(), &dir, "completed", Some(0), Some("t1")).unwrap();
        assert!(read(tmp.path(), &dir).unwrap().goal.starts_with("Revised:"));
    }

    #[test]
    fn the_goal_stops_at_the_next_heading() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = create(tmp.path(), args("LR ablation", "python run.py")).unwrap();
        let goal = read(tmp.path(), &dir).unwrap().goal;
        assert!(!goal.contains("## Command"), "{goal:?}");
        assert!(!goal.contains("python run.py"), "{goal:?}");
    }

    #[test]
    fn ordering_survives_the_fourth_digit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(DIR);
        // Past 999 the prefix widens, and "1000" sorts before "999" as a string.
        for (prefix, slug) in [(998, "early"), (999, "later"), (1000, "latest")] {
            let dir = root.join(format!("{prefix:03}-{slug}"));
            fs::create_dir_all(&dir).unwrap();
            let record = Record {
                uuid: format!("u{prefix}"),
                name: slug.to_string(),
                command: String::new(),
                goal: String::new(),
                status: "completed".to_string(),
                exit_code: Some(0),
                started_at: "t".to_string(),
                ended_at: None,
            };
            backfill(tmp.path(), &format!("{DIR}/{prefix:03}-{slug}"), &record, "/w", "\n# x\n")
                .unwrap();
        }
        let names: Vec<String> = list(tmp.path()).into_iter().map(|(_, r)| r.name).collect();
        assert_eq!(names, ["latest", "later", "early"]);
    }
}
