use std::path::Path;

use anyhow::{Context, Result};

/// Write `.ire/mcp.json` so Claude Code knows how to spawn the MCP server.
pub fn write_mcp_config(ire_dir: &Path, socket_path: &Path) -> Result<()> {
    let server_js = concat!(env!("IRE_MCP_DIR"), "/server.js");
    let workspace_root = ire_dir
        .parent()
        .unwrap_or(ire_dir)
        .to_string_lossy()
        .to_string();

    let config = serde_json::json!({
        "mcpServers": {
            "ire": {
                "command": "node",
                "args": [server_js],
                "env": {
                    "IRE_WORKSPACE": workspace_root,
                    "IRE_BACKEND_SOCKET": socket_path.to_string_lossy()
                }
            }
        }
    });

    let json = serde_json::to_string_pretty(&config)?;
    std::fs::write(ire_dir.join("mcp.json"), json).context("write mcp.json")?;
    Ok(())
}
