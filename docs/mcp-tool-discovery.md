# Why MCP tools aren't documented in `_SYSTEM.md`

When an agent (Claude Code, Codex) connects to IRE's bundled MCP server, the
MCP JSON-RPC handshake includes a `tools/list` call that returns each tool's
name, description, and input schema directly to the agent — see
[SDD §11](SDD.md#11-mcp-server).

This means tool descriptions are always in sync with the server code and need
no separate documentation. If you add or change an MCP tool, update its schema/description 
in `mcp/server.js` — that's the single source of truth agents see.
