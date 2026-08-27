use std::path::Path;

use crate::prompts::{self, WakeupArgs};

/// The message an agent needs to pick a finished experiment back up: its
/// goal, its exit code, and where to read the rest. Deliberately a pointer,
/// not a dump — the log tails used to be inlined here, but the agent can
/// read them on demand via the `experiment.tail_logs` MCP tool, and pasting
/// 16KB of them into the user's composer buried the actual instruction.
///
/// Purely read-only: composes text, sends nothing and resumes no session.
/// The user copies or drags this into whichever chat they want; sending it
/// from there is a normal chat turn, not a special "wake-up" path.
pub fn compose_message(workspace_root: &Path, uuid: &str, exit_code: i32, record_dir: &str) -> String {
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_points_at_the_record_instead_of_inlining_logs() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = super::super::record::create(
            tmp.path(),
            super::super::record::RecordArgs {
                uuid: "abcd",
                name: "a run",
                command: "python a.py",
                working_dir: "/tmp/proj",
                wake_prompt: "check whether loss drops",
                started_at: "2026-08-01T10:00:00+02:00",
            },
        )
        .unwrap();

        // Logs exist on disk; the message must still not carry their contents.
        let logs = tmp.path().join(".ire/cache/experiments/abcd");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("stdout.log"), "SECRET-LOG-BODY\n").unwrap();

        let msg = compose_message(tmp.path(), "abcd", 0, &dir);

        assert!(msg.contains("check whether loss drops"));
        assert!(msg.contains(&format!("{dir}/EXPERIMENT.md")));
        assert!(msg.contains("experiment.tail_logs"));
        assert!(msg.contains("experiment.mark_reviewed"));
        assert!(!msg.contains("SECRET-LOG-BODY"), "log bodies must not be inlined");
        assert!(!msg.contains('{'), "unsubstituted placeholder left in: {msg}");
    }
}
