use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Map, Value};

/// Name of the config-defined agent IRE injects a composed system prompt
/// into. Only registered (and only passed via `--agent`) when a turn actually
/// has a system prompt — MCP servers apply to every agent regardless (they
/// live under the config's top-level `mcp` key, not nested under `agent`), so
/// a turn with MCP config but no system prompt needs no `--agent` flag at all.
const IRE_AGENT_NAME: &str = "ire";

pub struct OpenCodeSpawnArgs<'a> {
    pub bin: &'a Path,
    pub workspace: &'a Path,
    pub message: &'a str,
    pub model: &'a str,
    pub effort: Option<&'a str>,
    pub resume_id: Option<&'a str>,
    pub mcp_config: Option<&'a Path>,
    pub system_prompt: Option<&'a str>,
}

pub fn build_opencode_command(args: &OpenCodeSpawnArgs<'_>) -> Command {
    let mut cmd = Command::new(args.bin);

    cmd.arg("run")
        .arg("--model")
        .arg(args.model)
        .arg("--format")
        .arg("json")
        // Required: without --auto, opencode blocks on a permission prompt
        // it has no TTY to ask (confirmed empirically — the process hangs
        // forever, not a quick failure), since IRE's own MCP-based approval
        // flow (ask_user_question) is a separate mechanism from opencode's
        // built-in one.
        .arg("--auto");

    if let Some(effort) = args.effort {
        cmd.arg("--variant").arg(effort);
    }

    if let Some(id) = args.resume_id {
        cmd.arg("--session").arg(id);
    }

    if let Some(config) = inject_config(args.mcp_config, args.system_prompt) {
        if args.system_prompt.is_some() {
            cmd.arg("--agent").arg(IRE_AGENT_NAME);
        }
        cmd.env("OPENCODE_CONFIG_CONTENT", config);
    }

    cmd.arg("--")
        .arg(args.message)
        .current_dir(args.workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd
}

/// Builds the `OPENCODE_CONFIG_CONTENT` env value (confirmed via
/// `@opencode-ai/sdk`'s own server wrapper, which uses the same env var to
/// pass inline config to a spawned `opencode` process — this avoids writing
/// any file into the user's actual workspace, unlike a project-local
/// `opencode.jsonc`, which opencode also merges but which would otherwise
/// mean silently dropping a config file into the user's git-tracked repo).
fn inject_config(mcp_config: Option<&Path>, system_prompt: Option<&str>) -> Option<String> {
    let mut root = Map::new();

    if let Some(path) = mcp_config {
        if let Some(mcp) = translate_mcp_servers(path) {
            if !mcp.is_empty() {
                root.insert("mcp".to_string(), Value::Object(mcp));
            }
        }
    }

    if let Some(prompt) = system_prompt {
        let mut ire_agent = Map::new();
        ire_agent.insert("mode".to_string(), Value::String("primary".to_string()));
        ire_agent.insert("prompt".to_string(), Value::String(prompt.to_string()));
        let mut agent = Map::new();
        agent.insert(IRE_AGENT_NAME.to_string(), Value::Object(ire_agent));
        root.insert("agent".to_string(), Value::Object(agent));
    }

    if root.is_empty() {
        return None;
    }
    serde_json::to_string(&Value::Object(root)).ok()
}

/// Reads IRE's own `mcp.json` (`{"mcpServers": {"<name>": {"command", "args",
/// "env"}}}`, the same file Claude gets verbatim via `--mcp-config`) and
/// re-expresses each server in OpenCode's `McpLocalConfig` shape
/// (`{"type": "local", "command": [bin, ...args], "environment": {...}}` —
/// note `command` is one array combining binary+args, not split like IRE's
/// own file, and the env key is named `environment`). Servers missing a
/// `command` are skipped individually rather than failing the whole
/// translation, same defensive style as Codex's `inject_mcp_servers`.
fn translate_mcp_servers(path: &Path) -> Option<Map<String, Value>> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;
    let servers = json["mcpServers"].as_object()?;

    let mut out = Map::new();
    for (name, cfg) in servers {
        let Some(command) = cfg["command"].as_str() else {
            continue;
        };

        let mut command_arr = vec![Value::String(command.to_string())];
        if let Some(extra_args) = cfg["args"].as_array() {
            command_arr.extend(extra_args.iter().cloned());
        }

        let mut server = Map::new();
        server.insert("type".to_string(), Value::String("local".to_string()));
        server.insert("command".to_string(), Value::Array(command_arr));
        if let Some(env) = cfg["env"].as_object() {
            server.insert("environment".to_string(), Value::Object(env.clone()));
        }

        out.insert(name.clone(), Value::Object(server));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn base_args<'a>(bin: &'a Path, workspace: &'a Path) -> OpenCodeSpawnArgs<'a> {
        OpenCodeSpawnArgs {
            bin,
            workspace,
            message: "hello",
            model: "anthropic/claude-opus-4-8",
            effort: None,
            resume_id: None,
            mcp_config: None,
            system_prompt: None,
        }
    }

    fn args_to_vec(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn fresh_turn_uses_run_model_json_auto_and_dash_dash_before_message() {
        let bin = PathBuf::from("/usr/local/bin/opencode");
        let workspace = PathBuf::from("/tmp/ws");
        let cmd = build_opencode_command(&base_args(&bin, &workspace));
        let args = args_to_vec(&cmd);
        assert_eq!(
            args,
            vec![
                "run", "--model", "anthropic/claude-opus-4-8", "--format", "json", "--auto", "--",
                "hello",
            ]
        );
    }

    #[test]
    fn resume_turn_passes_session_flag() {
        let bin = PathBuf::from("/usr/local/bin/opencode");
        let workspace = PathBuf::from("/tmp/ws");
        let mut a = base_args(&bin, &workspace);
        a.resume_id = Some("ses_abc123");
        let cmd = build_opencode_command(&a);
        let args = args_to_vec(&cmd);
        assert!(args.windows(2).any(|w| w == ["--session", "ses_abc123"]));
    }

    #[test]
    fn effort_maps_to_variant_flag() {
        let bin = PathBuf::from("/usr/local/bin/opencode");
        let workspace = PathBuf::from("/tmp/ws");
        let mut a = base_args(&bin, &workspace);
        a.effort = Some("high");
        let cmd = build_opencode_command(&a);
        let args = args_to_vec(&cmd);
        assert!(args.windows(2).any(|w| w == ["--variant", "high"]));
    }

    #[test]
    fn system_prompt_registers_ire_agent_and_passes_agent_flag() {
        let bin = PathBuf::from("/usr/local/bin/opencode");
        let workspace = PathBuf::from("/tmp/ws");
        let mut a = base_args(&bin, &workspace);
        a.system_prompt = Some("be terse");
        let cmd = build_opencode_command(&a);
        let args = args_to_vec(&cmd);
        assert!(args.windows(2).any(|w| w == ["--agent", "ire"]));

        let env_val = cmd
            .get_envs()
            .find(|(k, _)| *k == std::ffi::OsStr::new("OPENCODE_CONFIG_CONTENT"))
            .and_then(|(_, v)| v)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let parsed: Value = serde_json::from_str(&env_val).unwrap();
        assert_eq!(parsed["agent"]["ire"]["prompt"], "be terse");
        assert_eq!(parsed["agent"]["ire"]["mode"], "primary");
    }

    #[test]
    fn mcp_only_config_does_not_pass_agent_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let mcp_path = tmp.path().join("mcp.json");
        std::fs::write(
            &mcp_path,
            r#"{"mcpServers": {"ire": {"command": "/bin/ire", "args": ["--mcp-stdio"], "env": {"FOO": "bar"}}}}"#,
        )
        .unwrap();

        let bin = PathBuf::from("/usr/local/bin/opencode");
        let workspace = PathBuf::from("/tmp/ws");
        let mut a = base_args(&bin, &workspace);
        a.mcp_config = Some(&mcp_path);
        let cmd = build_opencode_command(&a);
        let args = args_to_vec(&cmd);
        assert!(!args.iter().any(|s| s == "--agent"));

        let env_val = cmd
            .get_envs()
            .find(|(k, _)| *k == std::ffi::OsStr::new("OPENCODE_CONFIG_CONTENT"))
            .and_then(|(_, v)| v)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let parsed: Value = serde_json::from_str(&env_val).unwrap();
        assert_eq!(
            parsed["mcp"]["ire"]["command"],
            Value::Array(vec![
                Value::String("/bin/ire".to_string()),
                Value::String("--mcp-stdio".to_string()),
            ])
        );
        assert_eq!(parsed["mcp"]["ire"]["environment"]["FOO"], "bar");
        assert_eq!(parsed["mcp"]["ire"]["type"], "local");
    }

    #[test]
    fn malformed_mcp_config_is_silently_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        let mcp_path = tmp.path().join("mcp.json");
        std::fs::write(&mcp_path, "not json").unwrap();

        let bin = PathBuf::from("/usr/local/bin/opencode");
        let workspace = PathBuf::from("/tmp/ws");
        let mut a = base_args(&bin, &workspace);
        a.mcp_config = Some(&mcp_path);
        let cmd = build_opencode_command(&a);
        assert!(!cmd
            .get_envs()
            .any(|(k, _)| k == std::ffi::OsStr::new("OPENCODE_CONFIG_CONTENT")));
    }
}
