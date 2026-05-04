use std::path::Path;
use std::process::{Command, Stdio};

pub struct SpawnArgs<'a> {
    pub bin: &'a Path,
    pub workspace: &'a Path,
    pub message: &'a str,
    pub resume_id: Option<&'a str>,
    pub mcp_config: Option<&'a Path>,
    pub system_prompt: Option<&'a str>,
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
        .arg("bypassPermissions");

    if let Some(p) = args.mcp_config {
        cmd.arg("--mcp-config").arg(p);
    }

    if let Some(s) = args.system_prompt {
        cmd.arg("--append-system-prompt").arg(s);
    }

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
