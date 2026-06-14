# MCP Server

A Node.js stdio MCP server bundled at `mcp/server.js`. Spawned by Tauri at workspace open and torn down on close. Claude Code connects to it via `--mcp-config .ire/mcp.json`; Codex receives the same config translated to `-c mcp_servers.*` flags.

`.ire/mcp.json` is generated at workspace open:

```json
{
  "mcpServers": {
    "ire": {
      "command": "node",
      "args": ["<bundled>/mcp/server.js"],
      "env": {
        "IRE_WORKSPACE": "<absolute-path>",
        "IRE_BACKEND_SOCKET": "<unix-socket-path-or-tcp>"
      }
    }
  }
}
```

The MCP server is a **thin RPC bridge** to the Rust backend over a Unix domain socket (Windows: TCP on 127.0.0.1 with auth token). All real work — atomic writes, DB inserts, subprocess spawning — happens in Rust. The MCP server validates inputs against JSON schemas and forwards.

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

All tools return JSON. Errors are surfaced to CC as MCP error responses, which CC interprets as tool failures and reports in chat.

---

## Backend RPC Channel

The Node MCP server speaks line-delimited JSON over the socket:
```
→ { "id": 1, "method": "wiki.write", "params": { "path": "...", "content": "..." } }
← { "id": 1, "ok": true, "result": {} }
```
This is a private internal protocol; not part of any spec. It exists only because the MCP SDK is Node-only and we want all I/O to happen in Rust for atomicity.

---

## Why tool descriptions don't live in `_SYSTEM.md`

When an agent connects to the MCP server, the MCP JSON-RPC handshake includes a `tools/list` call that returns each tool's name, description, and input schema directly to the agent. Tool descriptions are therefore always in sync with the server code and need no separate documentation.

If you add or change an MCP tool, update its schema/description in `mcp/server.js` — that's the single source of truth agents see.
