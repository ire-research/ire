# MCP Server

An out-of-process Rust stdio MCP server: the `ire` binary re-invoked as `ire --mcp-stdio` (see `src-tauri/src/mcp/stdio_server.rs`). Spawned by Claude Code / Codex per session — not by Tauri. Claude Code connects to it via `--mcp-config ~/.ire/workspaces/<id>/mcp.json`; Codex receives the same config translated to `-c mcp_servers.*` flags.

`mcp.json` (under the workspace's home data dir `~/.ire/workspaces/<id>/`) is generated at workspace open. The command is the running app's own executable path (`std::env::current_exe()`), so it needs no Node runtime and no build-time path:

```json
{
  "mcpServers": {
    "ire": {
      "command": "<abs-path-to-ire-binary>",
      "args": ["--mcp-stdio"],
      "env": {
        "IRE_WORKSPACE": "<absolute-path>",
        "IRE_BACKEND_SOCKET": "<unix-socket-path-or-tcp>"
      }
    }
  }
}
```

The MCP server is a **thin RPC bridge** to the running app over a Unix domain socket (currently only implemented on Unix). It is a separate process from the app, so it reaches live app state — events, `register_ask`, the session manager — only through the socket. All real work — atomic writes, DB inserts, subprocess spawning — happens in the app process (`src-tauri/src/mcp/rpc.rs`). The stdio server only advertises the catalog and forwards each call.

---

## Tool Catalog

| Tool | Description |
|---|---|
| `ire.read({})` | Read `ire.json`. Returns `{ content, version }` (raw JSON + content-hash token). Must precede `ire.edit`. |
| `ire.edit({ old, new, version })` | Exact string-replacement edit of `ire.json` (like the built-in `Edit`). Fails on a stale/missing `version`, or if `old` is absent or non-unique; the result must stay valid against the schema. Emits the `notes`/`focus`/`ideas` `workspace-event` variants. Does not commit. |
| `resource.add({ markdown, title?, sources? })` | Simulated ingestion (no fetch): writes the agent-supplied markdown to a draft and opens an Approve/Discard preview tab. On Approve it lands at `resources/<slug>.md` with `sources` injected into frontmatter. |
| `memory.write_long_term({ section, content })` | Append to `.ire/long-term.md` under section. Does not commit. |
| `memory.write_short_term({ content })` | Append to today's `.ire/short-term/YYYY-MM-DD.md`. Does not commit. |
| `experiment.start({ name, command, working_dir?, wake_prompt })` | Spawn detached subprocess, return `{ uuid }`. |
| `experiment.status({ uuid })` | Return `{ status, exit_code?, started_at, ended_at? }`. |
| `experiment.tail_logs({ uuid, kb? })` | Tail of stdout/stderr from `.ire/cache/experiments/<uuid>/`. |
| `ask_user_question({ questions })` | Block until the user answers via the IRE UI; returns `{ answers: [{ header, answer }] }`. Replaces CC's built-in `AskUserQuestion`, which is passed `--disallowedTools` (see [chat-agents.md](chat-agents.md#agent-subprocess-layer)). |

Resources are otherwise read with the built-in `Read` tool against `.ire/resources/` (and its `_index.md`); there is no `wiki.*` or resource-read MCP tool, and `experiment.list` is dropped (read experiments from `ire.json` via `ire.read`).

All tools return JSON. Errors are surfaced to CC as MCP error responses, which CC interprets as tool failures and reports in chat.

---

## Backend RPC Channel

The stdio MCP server speaks line-delimited JSON over the socket:
```
→ { "id": 1, "method": "ire.edit", "params": { "old": "...", "new": "...", "version": "..." } }
← { "id": 1, "ok": true, "result": { "edited": "ire.json" } }
```
This is a private internal protocol; not part of any spec. It exists because the MCP server runs as a separate process from the app and needs a channel into live app state; all I/O happens in the app process for atomicity.

---

## Why tool descriptions don't live in `_SYSTEM.md`

When an agent connects to the MCP server, the MCP JSON-RPC handshake includes a `tools/list` call that returns each tool's name, description, and input schema directly to the agent. Tool descriptions are therefore always in sync with the server code and need no separate documentation.

If you add or change an MCP tool, update its schema/description in `tool_catalog()` in `src-tauri/src/mcp/stdio_server.rs` — that's the single source of truth agents see.
