use std::path::Path;

use anyhow::{Context, Result};

/// Write `mcp.json` into the workspace's home data dir so Claude Code knows how
/// to spawn the MCP server.
///
/// The server is this very binary re-invoked with `--mcp-stdio`, so the command
/// always resolves to the installed app — no Node runtime, no build-time path.
pub fn write_mcp_config(data_dir: &Path, workspace_root: &Path, socket_path: &Path) -> Result<()> {
    let exe = std::env::current_exe().context("resolve current executable")?;

    let config = serde_json::json!({
        "mcpServers": {
            "ire": {
                "command": exe.to_string_lossy(),
                "args": ["--mcp-stdio"],
                "env": {
                    "IRE_WORKSPACE": workspace_root.to_string_lossy(),
                    "IRE_BACKEND_SOCKET": socket_path.to_string_lossy()
                }
            }
        }
    });

    let json = serde_json::to_string_pretty(&config)?;
    std::fs::write(data_dir.join("mcp.json"), json).context("write mcp.json")?;
    Ok(())
}
