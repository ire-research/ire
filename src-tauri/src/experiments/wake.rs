use std::fs;
use std::path::Path;

use crate::prompts::{self, WakeupArgs};

/// The message an agent would need to pick a finished experiment back up:
/// its goal, exit code, and log tails. Purely read-only — composes text,
/// sends nothing and resumes no session. The user copies or drags this into
/// whichever chat they want (new or existing); sending it from there is a
/// normal chat turn like any other, not a special "wake-up" path.
pub fn compose_message(workspace_root: &Path, uuid: &str, exit_code: i32, record_dir: &str) -> String {
    let exp_dir = workspace_root.join(".ire/cache/experiments").join(uuid);
    let stdout_tail = tail_file(&exp_dir.join("stdout.log"), 8192);
    let stderr_tail = tail_file(&exp_dir.join("stderr.log"), 8192);

    // The goal comes back out of the record, the only place it is kept. A run
    // whose record has gone still composes a message, just without its context.
    let goal = super::record::read(workspace_root, record_dir)
        .map(|r| r.goal)
        .unwrap_or_default();
    let goal = if goal.is_empty() {
        format!("Experiment {uuid} finished. Its goal and context are in {record_dir}/EXPERIMENT.md.")
    } else {
        goal
    };

    prompts::experiment_wakeup(WakeupArgs {
        wake_prompt: &goal,
        uuid,
        exit_code,
        record_dir,
        stdout_tail: &stdout_tail,
        stderr_tail: &stderr_tail,
    })
}

fn tail_file(path: &Path, max_bytes: u64) -> String {
    let Ok(content) = fs::read(path) else {
        return String::new();
    };
    let len = content.len() as u64;
    let start = if len > max_bytes {
        (len - max_bytes) as usize
    } else {
        0
    };
    String::from_utf8_lossy(&content[start..]).into_owned()
}
