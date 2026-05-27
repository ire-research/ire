use std::path::Path;
use std::process::{Command, Stdio};

pub struct CodexSpawnArgs<'a> {
    pub bin: &'a Path,
    pub workspace: &'a Path,
    pub message: &'a str,
    pub model: &'a str,
    pub reasoning_effort: &'a str,
    pub system_prompt: Option<&'a str>,
    pub mcp_config: Option<&'a Path>,
    pub resume_id: Option<&'a str>,
}

pub fn build_codex_command(args: &CodexSpawnArgs<'_>) -> Command {
    let mut cmd = Command::new(args.bin);

    if args.resume_id.is_some() {
        cmd.arg("exec").arg("resume");
    } else {
        cmd.arg("exec");
    }

    cmd.arg("-m")
        .arg(args.model)
        .arg("-c")
        .arg(format!("model_reasoning_effort={}", args.reasoning_effort));

    if args.resume_id.is_none() {
        cmd.arg("-C").arg(args.workspace);
    }

    cmd.arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("--json");

    if let Some(prompt) = args.system_prompt {
        cmd.arg("-c")
            .arg(format!("developer_instructions={}", toml_string(prompt)));
    }

    if let Some(mcp_path) = args.mcp_config {
        inject_mcp_servers(&mut cmd, mcp_path);
    }

    if let Some(id) = args.resume_id {
        cmd.arg(id);
    }

    cmd.arg("--")
        .arg(args.message)
        .current_dir(args.workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd
}

fn inject_mcp_servers(cmd: &mut Command, path: &Path) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };
    let Some(servers) = json["mcpServers"].as_object() else {
        return;
    };

    for (name, cfg) in servers {
        if let Some(command) = cfg["command"].as_str() {
            cmd.arg("-c").arg(format!(
                "mcp_servers.{name}.command={}",
                toml_string(command)
            ));
        }

        if let Some(args_arr) = cfg["args"].as_array() {
            let args: Vec<toml::Value> = args_arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| toml::Value::String(s.to_string())))
                .collect();
            cmd.arg("-c").arg(format!(
                "mcp_servers.{name}.args={}",
                toml::Value::Array(args)
            ));
        }

        if let Some(env_obj) = cfg["env"].as_object() {
            for (key, val) in env_obj {
                if let Some(s) = val.as_str() {
                    cmd.arg("-c")
                        .arg(format!("mcp_servers.{name}.env.{key}={}", toml_string(s)));
                }
            }
        }
    }
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn fresh_command_injects_developer_instructions_and_mcp_servers() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        let mcp_path = workspace.join("mcp.json");
        std::fs::write(
            &mcp_path,
            serde_json::json!({
                "mcpServers": {
                    "ire": {
                        "command": "node",
                        "args": ["/tmp/server.js"],
                        "env": {
                            "IRE_WORKSPACE": "/tmp/workspace",
                            "IRE_BACKEND_SOCKET": "/tmp/ire.sock"
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();

        let cmd = build_codex_command(&CodexSpawnArgs {
            bin: Path::new("codex"),
            workspace,
            message: "hello",
            model: "gpt-5.3-codex",
            reasoning_effort: "high",
            system_prompt: Some("system prompt\nwith \"quotes\""),
            mcp_config: Some(&mcp_path),
            resume_id: None,
        });
        let args = command_args(&cmd);

        assert_eq!(args[0], "exec");
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-C", &workspace.to_string_lossy()]));
        assert!(args.windows(2).any(|pair| pair == ["-m", "gpt-5.3-codex"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-c", "model_reasoning_effort=high"]));
        assert!(args.windows(2).any(|pair| {
            pair == [
                "-c",
                &format!(
                    "developer_instructions={}",
                    toml_string("system prompt\nwith \"quotes\"")
                ),
            ]
        }));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-c", "mcp_servers.ire.command=\"node\""]));
        assert!(args
            .windows(2)
            .any(|pair| { pair == ["-c", "mcp_servers.ire.args=[\"/tmp/server.js\"]"] }));
        assert!(args.windows(2).any(|pair| {
            pair == ["-c", "mcp_servers.ire.env.IRE_WORKSPACE=\"/tmp/workspace\""]
        }));
        assert!(args.windows(2).any(|pair| {
            pair == [
                "-c",
                "mcp_servers.ire.env.IRE_BACKEND_SOCKET=\"/tmp/ire.sock\"",
            ]
        }));
        assert_eq!(
            &args[args.len() - 2..],
            ["--".to_string(), "hello".to_string()]
        );
    }

    #[test]
    fn resume_command_uses_resume_subcommand_without_cd_flag() {
        let cmd = build_codex_command(&CodexSpawnArgs {
            bin: Path::new("codex"),
            workspace: Path::new("/tmp/workspace"),
            message: "continue",
            model: "gpt-5.3-codex",
            reasoning_effort: "medium",
            system_prompt: None,
            mcp_config: None,
            resume_id: Some("thread-123"),
        });
        let args = command_args(&cmd);

        assert_eq!(args[0], "exec");
        assert_eq!(args[1], "resume");
        assert!(!args.iter().any(|arg| arg == "-C"));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-c", "model_reasoning_effort=medium"]));
        assert_eq!(
            &args[args.len() - 3..],
            [
                "thread-123".to_string(),
                "--".to_string(),
                "continue".to_string(),
            ]
        );
    }

    #[test]
    fn prompt_separator_allows_messages_that_start_with_dash() {
        let cmd = build_codex_command(&CodexSpawnArgs {
            bin: Path::new("codex"),
            workspace: Path::new("/tmp/workspace"),
            message: "- continue from this bullet",
            model: "gpt-5.3-codex",
            reasoning_effort: "medium",
            system_prompt: None,
            mcp_config: None,
            resume_id: None,
        });
        let args = command_args(&cmd);

        assert_eq!(
            &args[args.len() - 2..],
            ["--".to_string(), "- continue from this bullet".to_string()]
        );
    }
}
