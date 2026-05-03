use std::path::Path;
use std::process::{Command, Stdio};

pub struct SpawnArgs<'a> {
    pub bin: &'a Path,
    pub workspace: &'a Path,
    pub message: &'a str,
    pub mode: &'a str,
    pub resume_id: Option<&'a str>,
}

pub fn build_command(args: &SpawnArgs<'_>) -> Command {
    let mut cmd = Command::new(args.bin);

    cmd.arg("-p")
        .arg(args.message)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--include-partial-messages")
        .arg("--permission-mode")
        .arg(if args.mode == "experiment" { "acceptEdits" } else { "default" });

    if let Some(id) = args.resume_id {
        cmd.arg("--resume").arg(id);
    }

    cmd.current_dir(args.workspace)
        .env_remove("CLAUDECODE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd
}
