# MCP Server

An in-process Rust stdio MCP server: the `ire` binary re-invoked as `ire --mcp-stdio` (see `src-tauri/src/mcp/stdio_server.rs`). Spawned by Claude Code / Codex per session — not by Tauri. Claude Code connects to it via `--mcp-config .ire/mcp.json`; Codex receives the same config translated to `-c mcp_servers.*` flags.

`.ire/mcp.json` is generated at workspace open. The command is the running app's own executable path (`std::env::current_exe()`), so it needs no Node runtime and no build-time path:

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

The MCP server is a **thin RPC bridge** to the running app over a Unix domain socket (Windows: TCP on 127.0.0.1 with auth token). It is a separate process from the app, so it reaches live app state — events, `register_ask`, the session manager — only through the socket. All real work — atomic writes, DB inserts, subprocess spawning — happens in the app process (`src-tauri/src/mcp/rpc.rs`). The stdio server only advertises the catalog and forwards each call.

---

## Tool Catalog (MVP)

| Tool | Description |
|---|---|
| `wiki.read({ path })` | Read any wiki markdown or JSON file. Returns content + frontmatter for markdown. |
| `wiki.write({ path, content, summary? })` | Atomic write; updates `_index.md`; dispatches a typed `workspace-event` variant (and for `resources/*.md`, links the DB row + emits `resource-changed`). Does not commit. |
| `wiki.append({ path, content })` | Append content to a wiki file. Same persistence semantics as `wiki.write`. |
| `wiki.list({ glob? })` | List wiki paths; defaults to all. |
| `wiki.rename({ from, to })` | Atomic rename + index update. Does not commit. |
| `memory.write_long_term({ section, content })` | Append to `long-term.md` under section. Does not commit. |
| `memory.write_short_term({ content })` | Append to today's `short-term/YYYY-MM-DD.md`. Does not commit. |
| `pulse.update({ research_question?, this_week? })` | Patch `pulse.json`. Does not commit. |
| `resource.fetch({ url })` | Fetch URL, extract text, return it (does not save to wiki). |
| `experiment.start({ name, command, working_dir?, wake_prompt })` | Spawn detached subprocess, return `{ uuid }`. |
| `experiment.status({ uuid })` | Return `{ status, exit_code?, started_at, ended_at? }`. |
| `experiment.list({ limit? })` | Recent experiments. |
| `experiment.tail_logs({ uuid, kb? })` | Tail of stdout/stderr. |
| `ask_user_question({ questions })` | Block until the user answers via the IRE UI; returns `{ answers: [{ header, answer }] }`. Replaces CC's built-in `AskUserQuestion`, which is passed `--disallowedTools` (see [chat-agents.md](chat-agents.md#agent-subprocess-layer)). |

All tools return JSON. Errors are surfaced to CC as MCP error responses, which CC interprets as tool failures and reports in chat.

---

## Backend RPC Channel

The Node MCP server speaks line-delimited JSON over the socket:
```
→ { "id": 1, "method": "wiki.write", "params": { "path": "...", "content": "..." } }
← { "id": 1, "ok": true, "result": {} }
```
This is a private internal protocol; not part of any spec. It exists because the MCP server runs as a separate process from the app and needs a channel into live app state; all I/O happens in the app process for atomicity.

---

## Why tool descriptions don't live in `_SYSTEM.md`

When an agent connects to the MCP server, the MCP JSON-RPC handshake includes a `tools/list` call that returns each tool's name, description, and input schema directly to the agent. Tool descriptions are therefore always in sync with the server code and need no separate documentation.

If you add or change an MCP tool, update its schema/description in `tool_catalog()` in `src-tauri/src/mcp/stdio_server.rs` — that's the single source of truth agents see.
