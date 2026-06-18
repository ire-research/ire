// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Hidden mode: Claude Code / Codex spawn this same binary as the MCP server.
    if std::env::args().any(|arg| arg == "--mcp-stdio") {
        ire_lib::run_mcp_stdio();
        return;
    }
    ire_lib::run()
}
