use std::path::Path;

use serde_json::{Map, Value};

/// Env var read by `mcp::stdio_server`: when set, the IRE MCP server hides
/// (and rejects calls to) `ask_user_question`. OpenCode's own native
/// `question` tool replaces it for OpenCode turns — see
/// docs/opencode-server-integration.md "Native questions, not IRE's MCP
/// question tool". Only ever set on the MCP subprocess(es) OpenCode's own
/// server spawns, never on Claude/Codex's `mcp.json`.
const EXCLUDE_ASK_ENV: &str = "IRE_MCP_EXCLUDE_ASK";

/// Builds the `OPENCODE_CONFIG_CONTENT` env value for the `opencode serve`
/// process IRE starts for one workspace (confirmed via `@opencode-ai/sdk`'s
/// own server wrapper, which uses the same env var to pass inline config to a
/// spawned `opencode` process — this avoids writing any file into the user's
/// actual workspace, unlike a project-local `opencode.jsonc`, which opencode
/// also merges but which would otherwise mean silently dropping a config file
/// into the user's git-tracked repo).
///
/// Unlike the old per-turn CLI invocation, this overlay is built once at
/// server startup: it carries no per-turn system prompt (that's passed
/// per-message via `prompt_async`'s `system` field instead — see
/// `opencode::turn` — so a stale workspace context can never become
/// persistent server configuration).
pub fn server_config(mcp_config: Option<&Path>) -> String {
    let mut root = Map::new();

    // Equivalent to the old CLI transport's `--auto`: the server has no TTY
    // to ask a permission prompt on, and IRE doesn't yet ship its own
    // permission-approval UI (see docs/opencode-server-integration.md
    // "Non-goals").
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
