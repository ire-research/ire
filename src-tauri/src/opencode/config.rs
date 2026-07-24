use std::path::Path;

use serde_json::{Map, Value};

/// Read by `mcp::stdio_server` to hide/reject `ask_user_question` — set only
/// on OpenCode's own MCP subprocess, never Claude/Codex's `mcp.json`.
const EXCLUDE_ASK_ENV: &str = "IRE_MCP_EXCLUDE_ASK";

/// Builds `OPENCODE_CONFIG_CONTENT` for the `opencode serve` process IRE
/// starts per workspace. No file is written into the user's workspace, and
/// no per-turn system prompt is baked in (that's per-message instead).
pub fn server_config(mcp_config: Option<&Path>) -> String {
    let mut root = Map::new();

    // Equivalent to the old CLI transport's `--auto`.
    root.insert(
        "permission".to_string(),
        Value::String("allow".to_string()),
    );

    if let Some(path) = mcp_config {
        if let Some(mcp) = translate_mcp_servers(path) {
            if !mcp.is_empty() {
                root.insert("mcp".to_string(), Value::Object(mcp));
            }
        }
    }

    serde_json::to_string(&Value::Object(root)).unwrap_or_default()
}

/// Translates IRE's `mcp.json` into OpenCode's `McpLocalConfig` shape.
/// Servers missing a `command` are skipped rather than failing the batch.
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

        let mut environment = cfg["env"].as_object().cloned().unwrap_or_default();
        environment.insert(EXCLUDE_ASK_ENV.to_string(), Value::String("1".to_string()));

        let mut server = Map::new();
        server.insert("type".to_string(), Value::String("local".to_string()));
        server.insert("command".to_string(), Value::Array(command_arr));
        server.insert("environment".to_string(), Value::Object(environment));

        out.insert(name.clone(), Value::Object(server));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_mcp_config_still_sets_allow_permission() {
        let config = server_config(None);
        let parsed: Value = serde_json::from_str(&config).unwrap();
        assert_eq!(parsed["permission"], "allow");
        assert!(parsed.get("mcp").is_none());
    }

    #[test]
    fn mcp_config_is_translated_and_ask_is_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        let mcp_path = tmp.path().join("mcp.json");
        std::fs::write(
            &mcp_path,
            r#"{"mcpServers": {"ire": {"command": "/bin/ire", "args": ["--mcp-stdio"], "env": {"FOO": "bar"}}}}"#,
        )
        .unwrap();

        let config = server_config(Some(&mcp_path));
        let parsed: Value = serde_json::from_str(&config).unwrap();
        assert_eq!(parsed["permission"], "allow");
        assert_eq!(
            parsed["mcp"]["ire"]["command"],
            Value::Array(vec![
                Value::String("/bin/ire".to_string()),
                Value::String("--mcp-stdio".to_string()),
            ])
        );
        assert_eq!(parsed["mcp"]["ire"]["type"], "local");
        assert_eq!(parsed["mcp"]["ire"]["environment"]["FOO"], "bar");
        assert_eq!(parsed["mcp"]["ire"]["environment"]["IRE_MCP_EXCLUDE_ASK"], "1");
    }

    #[test]
    fn malformed_mcp_config_is_silently_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        let mcp_path = tmp.path().join("mcp.json");
        std::fs::write(&mcp_path, "not json").unwrap();

        let config = server_config(Some(&mcp_path));
        let parsed: Value = serde_json::from_str(&config).unwrap();
        assert!(parsed.get("mcp").is_none());
    }
}
