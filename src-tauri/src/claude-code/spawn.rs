use std::path::Path;
use std::process::{Command, Stdio};

pub struct SpawnArgs<'a> {
    pub bin: &'a Path,
    pub workspace: &'a Path,
    pub message: &'a str,
    pub resume_id: Option<&'a str>,
    pub mcp_config: Option<&'a Path>,
    pub system_prompt: Option<&'a str>,
    pub model: &'a str,
    pub effort: Option<&'a str>,
}

pub fn build_command(args: &SpawnArgs<'_>) -> Command {
    let mut cmd = Command::new(args.bin);

    cmd.arg("-p")
        .arg(args.message)
        .arg("--model")
        .arg(args.model);

    if let Some(effort) = args.effort {
        cmd.arg("--effort").arg(effort);
    }

    cmd.arg("--output-format")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn command_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn build_command_includes_effort_when_present() {
        let cmd = build_command(&SpawnArgs {
            bin: Path::new("claude"),
            workspace: Path::new("/tmp/workspace"),
            message: "hello",
            resume_id: None,
            mcp_config: None,
            system_prompt: None,
            model: "claude-sonnet-4-6",
            effort: Some("high"),
        });
        let args = command_args(&cmd);

        assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
    }

    #[test]
    fn build_command_omits_effort_when_absent() {
        let cmd = build_command(&SpawnArgs {
            bin: Path::new("claude"),
            workspace: Path::new("/tmp/workspace"),
            message: "hello",
            resume_id: None,
            mcp_config: None,
            system_prompt: None,
            model: "claude-haiku-4-5-20251001",
            effort: None,
        });
        let args = command_args(&cmd);

        assert!(!args.iter().any(|arg| arg == "--effort"));
    }
}
