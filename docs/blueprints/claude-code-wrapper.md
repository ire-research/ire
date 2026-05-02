# Building a Custom GUI Wrapper for Claude Code

This document explains exactly how Tolaria wraps the Claude Code CLI inside a Tauri desktop app to produce a custom AI chat/agent experience. Follow these steps to replicate the pattern in your own app.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Step 1 — Locate the Claude Binary](#step-1--locate-the-claude-binary)
3. [Step 2 — Spawn the Subprocess](#step-2--spawn-the-subprocess)
4. [Step 3 — Parse the NDJSON Stream](#step-3--parse-the-ndjson-stream)
5. [Step 4 — Bridge Events to the Frontend](#step-4--bridge-events-to-the-frontend)
6. [Step 5 — (Optional) Add an MCP Server for App-Specific Tools](#step-5--optional-add-an-mcp-server-for-app-specific-tools)
7. [Step 6 — Build the React Frontend](#step-6--build-the-react-frontend)
8. [Gotchas and Edge Cases](#gotchas-and-edge-cases)
9. [Minimal End-to-End Example](#minimal-end-to-end-example)

---

## Architecture Overview

```
React UI
  │  invoke('stream_agent', {message, ...})
  ▼
Tauri Command (Rust)
  │  spawn_blocking → claude -p "..." --output-format stream-json ...
  ▼
Claude Code CLI subprocess
  │  stdout: newline-delimited JSON (NDJSON)
  ▼
Rust event parser
  │  app_handle.emit("agent-stream", ClaudeStreamEvent)
  ▼
React event listener
  │  listen('agent-stream', handler)
  ▼
UI state update → re-render
```

The Claude Code CLI is treated as a headless subprocess. It knows nothing about your GUI. Your app provides a thin IPC bridge: messages in, typed events out.

---

## Step 1 — Locate the Claude Binary

Claude Code is installed in many places depending on the user's platform and toolchain. You must search all of them — `which claude` only works if the GUI's `PATH` matches the shell's `PATH`, which it usually does not on macOS and Linux.

### Search strategy (priority order)

1. **`which`/`where` on the current process PATH**
2. **User's login shell** — spawn `$SHELL -lc "command -v claude"` to load shell init files that set up nvm, mise, asdf, etc.
3. **Well-known candidate paths** — check these directly:

```
~/.local/bin/claude
~/.claude/local/claude          ← default self-install location
~/.local/share/mise/shims/claude
~/.asdf/shims/claude
~/.npm-global/bin/claude
~/.npm/bin/claude
~/.linuxbrew/bin/claude
~/.nvm/versions/node/*/bin/claude   ← enumerate all nvm versions
/opt/homebrew/bin/claude
/usr/local/bin/claude
~/AppData/Roaming/npm/claude.cmd    ← Windows
~/AppData/Local/pnpm/claude.cmd
~/scoop/shims/claude.exe
```

### Rust implementation sketch

```rust
fn find_claude_binary() -> Result<PathBuf, String> {
    // 1. which/where
    if let Some(path) = run_command("which", &["claude"]) {
        return Ok(path);
    }

    // 2. Login shell
    let shell = std::env::var("SHELL").unwrap_or("/bin/bash".into());
    if let Some(path) = run_command(&shell, &["-lc", "command -v claude"]) {
        return Ok(path);
    }

    // 3. Candidate list
    for candidate in candidate_paths() {
        if candidate.exists() && is_executable(&candidate) {
            return Ok(candidate);
        }
    }

    Err("Claude CLI not found. Run: npm install -g @anthropic-ai/claude-code".into())
}
```

> **Why the login shell matters:** on macOS, GUI apps launch without `/etc/profile` or `~/.zshrc`. A user who installed claude via nvm will have it on their shell `PATH` but not on the GUI app's `PATH`. The `-lc` flag forces the shell to load its init files.

---

## Step 2 — Spawn the Subprocess

### Chat mode (no tools)

```
claude -p "<message>"
  --output-format stream-json
  --verbose
  --include-partial-messages
  --tools ""
  [--system-prompt "<text>"]
  [--resume "<session-id>"]
```

- `--tools ""` disables all built-in tools so Claude just responds in text.
- `--resume` enables multi-turn conversations by resuming a prior session.

### Agent mode (with tools and MCP)

```
claude -p "<message>"
  --output-format stream-json
  --verbose
  --include-partial-messages
  --mcp-config "<path-to-json>"
  --strict-mcp-config
  --permission-mode acceptEdits
  --tools "Read,Edit,MultiEdit,Write,Glob,Grep,LS"
  --no-session-persistence
  [--allowedTools "Bash"]            ← power-user mode only
  [--append-system-prompt "<text>"]
```

- `--mcp-config` points to a JSON file describing your app's MCP server (see Step 5).
- `--strict-mcp-config` prevents Claude from loading any other MCP config it may find on disk.
- `--permission-mode acceptEdits` auto-approves file edits without interactive prompts.
- `--no-session-persistence` prevents Claude from writing `.claude/` state files in the working directory.
- `--allowedTools Bash` pre-approves Bash so Claude never prompts; only include this for power-user scenarios.

### Process configuration

```rust
let mut cmd = Command::new(&binary);
cmd.args(&args)
   .env_remove("CLAUDECODE")   // ← critical: prevents "nested session" guard
   .stdin(Stdio::null())        // ← critical: Claude must not wait for stdin
   .stdout(Stdio::piped())
   .stderr(Stdio::piped());

if let Some(dir) = working_dir {
    cmd.current_dir(dir);       // ← set to user's project/vault root
}

// Extend PATH so Claude can find node, git, etc.
let path = augmented_path(&binary);
cmd.env("PATH", path);
```

**The three non-negotiable settings:**

| Setting | Reason |
|---|---|
| `env_remove("CLAUDECODE")` | Claude sets this env var when running. Without removing it, a nested invocation will refuse to start thinking it is already inside a session. |
| `stdin(Stdio::null())` | Claude waits for stdin in interactive mode. You must explicitly close it or the subprocess will hang. |
| `current_dir(project_path)` | Claude resolves relative file paths against the working directory. Setting it to the user's project root makes `Read`, `Write`, etc. work correctly. |

### Augmenting PATH

GUI apps inherit a minimal `PATH`. Extend it with common toolchain locations so Claude can find `node`, `git`, and other tools it may need:

```rust
fn augmented_path(binary: &Path) -> OsString {
    let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();

    // Add the directory containing the binary itself
    if let Some(parent) = binary.parent() {
        if !paths.contains(&parent.to_path_buf()) {
            paths.push(parent.to_path_buf());
        }
    }

    // Add common toolchain dirs
    let home = dirs::home_dir().unwrap_or_default();
    for extra in [
        home.join(".local/bin"),
        home.join(".bun/bin"),
        home.join(".npm-global/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ] {
        if !paths.contains(&extra) {
            paths.push(extra);
        }
    }

    std::env::join_paths(paths).unwrap_or_default()
}
```

---

## Step 3 — Parse the NDJSON Stream

Claude Code outputs one JSON object per line on stdout. Each line is a complete, self-contained event. Read stdout line-by-line and dispatch based on `type`.

### Event types you need to handle

```
{"type":"system","subtype":"init","session_id":"abc123"}
  → session established; save session_id for --resume

{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}}
  → streaming text chunk

{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"I should..."}}}
  → extended thinking chunk

{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"tool_use","id":"tu_1","name":"Read","input":{}}}}
  → tool call starting

{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}}
  → tool input streaming in (accumulate these per tool_id)

{"type":"tool_progress","tool_name":"Read","tool_use_id":"tu_1"}
  → alternative tool-start signal (agent mode)

{"type":"tool_result","tool_use_id":"tu_1","content":"file content here"}
  → tool finished, result available

{"type":"assistant","message":{"content":[{"type":"text","text":"..."},{"type":"tool_use",...}]}}
  → complete message (fallback if streaming didn't happen)

{"type":"result","result":"Final answer","session_id":"abc123"}
  → conversation turn complete

{"type":"result","subtype":"error_max_turns"}
  → Claude hit its turn limit
```

### Rust parser skeleton

```rust
fn dispatch_event(json: &Value, state: &mut StreamState, emit: &mut impl FnMut(Event)) {
    match json["type"].as_str().unwrap_or("") {
        "system" if json["subtype"] == "init" => {
            state.session_id = json["session_id"].as_str().unwrap_or("").into();
            emit(Event::Init { session_id: state.session_id.clone() });
        }
        "stream_event" => {
            let event = &json["event"];
            match event["type"].as_str().unwrap_or("") {
                "content_block_start" => {
                    let block = &event["content_block"];
                    if block["type"] == "tool_use" {
                        let id = block["id"].as_str().unwrap_or("").to_string();
                        let name = block["name"].as_str().unwrap_or("").to_string();
                        state.current_tool_id = Some(id.clone());
                        emit(Event::ToolStart { tool_name: name, tool_id: id });
                    }
                }
                "content_block_delta" => {
                    let delta = &event["delta"];
                    match delta["type"].as_str() {
                        Some("text_delta") => {
                            emit(Event::TextDelta { text: delta["text"].as_str().unwrap_or("").into() });
                        }
                        Some("thinking_delta") => {
                            emit(Event::ThinkingDelta { text: delta["thinking"].as_str().unwrap_or("").into() });
                        }
                        Some("input_json_delta") => {
                            // Accumulate tool input chunks — do NOT emit yet
                            if let (Some(partial), Some(ref tid)) = (delta["partial_json"].as_str(), &state.current_tool_id) {
                                state.tool_inputs.entry(tid.clone()).or_default().push_str(partial);
                            }
                        }
                        _ => {}
                    }
                }
                "content_block_stop" => { state.current_tool_id = None; }
                _ => {}
            }
        }
        "tool_result" => {
            if let Some(id) = json["tool_use_id"].as_str() {
                emit(Event::ToolDone { tool_id: id.into(), output: extract_output(json) });
            }
        }
        "result" => {
            state.session_id = json["session_id"].as_str().unwrap_or(&state.session_id).into();
            // Only emit text if streaming didn't already send it
            if !state.emitted_text {
                emit(Event::Result { text: json["result"].as_str().unwrap_or("").into() });
            }
        }
        _ => {}
    }
}
```

> **Tool input accumulation:** `input_json_delta` chunks arrive piecemeal. Collect them in a `HashMap<tool_id, String>` and use the accumulated value when you later need to show what the tool was called with. The final `assistant` message block also carries the complete `input` object as a fallback.

> **Deduplication:** If streaming worked, the `result` event's text field will be a duplicate of what you already rendered via `TextDelta`. Track `emitted_text: bool` in state and skip the `result` text when it's already been streamed.

---

## Step 4 — Bridge Events to the Frontend

### In Tauri (Rust → WebView)

Wrap the blocking subprocess in `tokio::task::spawn_blocking`. Emit a Tauri event for each parsed `ClaudeStreamEvent`:

```rust
#[tauri::command]
pub async fn stream_agent(
    app_handle: tauri::AppHandle,
    request: AgentRequest,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        run_agent(request, move |event| {
            let _ = app_handle.emit("agent-stream", &event);
        })
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}
```

> **Why `spawn_blocking`?** Reading subprocess stdout is synchronous I/O. Doing it on Tokio's async executor would block the runtime. `spawn_blocking` moves it to a dedicated thread pool.

Register the command in `tauri::Builder`:

```rust
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![stream_agent])
    .run(tauri::generate_context!())
    .unwrap();
```

### In React (TypeScript)

```typescript
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

type StreamEvent =
  | { kind: 'Init'; session_id: string }
  | { kind: 'TextDelta'; text: string }
  | { kind: 'ThinkingDelta'; text: string }
  | { kind: 'ToolStart'; tool_name: string; tool_id: string; input?: string }
  | { kind: 'ToolDone'; tool_id: string; output?: string }
  | { kind: 'Error'; message: string }
  | { kind: 'Done' }

async function sendMessage(message: string, vaultPath: string): Promise<void> {
  let closed = false

  const unlisten = await listen<StreamEvent>('agent-stream', (event) => {
    const data = event.payload
    if (data.kind === 'Done') {
      if (!closed) { closed = true; onDone() }
      return
    }
    handleEvent(data)
  })

  try {
    await invoke('stream_agent', {
      request: { message, vault_path: vaultPath }
    })
    if (!closed) { closed = true; onDone() }
  } catch (err) {
    onError(String(err))
  } finally {
    unlisten()  // always clean up the event listener
  }
}
```

> **Why listen before invoke?** If you set up the listener after `invoke`, you may miss events that arrive before the `await listen(...)` resolves. Always register the listener first.

> **Why unlisten in `finally`?** Tauri event listeners are global. If you don't remove them, they accumulate and multiple conversations will interfere with each other.

---

## Step 5 — (Optional) Add an MCP Server for App-Specific Tools

If you want Claude to interact with your app's data (not just the filesystem), expose an MCP server. Claude Code connects to it via the `--mcp-config` flag.

### MCP config JSON

Pass this inline (or write to a temp file) via `--mcp-config`:

```json
{
  "mcpServers": {
    "my-app": {
      "command": "node",
      "args": ["/path/to/mcp-server/index.js"],
      "env": {
        "PROJECT_PATH": "/user/project/dir"
      }
    }
  }
}
```

### Minimal Node.js MCP server

```js
import { Server } from '@modelcontextprotocol/sdk/server/index.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js'

const server = new Server({ name: 'my-app', version: '1.0.0' }, {
  capabilities: { tools: {} }
})

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [{
    name: 'search_items',
    description: 'Search items in the project',
    inputSchema: {
      type: 'object',
      properties: { query: { type: 'string' } },
      required: ['query']
    }
  }]
}))

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params
  if (name === 'search_items') {
    const results = await myApp.search(args.query)
    return { content: [{ type: 'text', text: JSON.stringify(results) }] }
  }
  throw new Error(`Unknown tool: ${name}`)
})

const transport = new StdioServerTransport()
await server.connect(transport)
```

### Optional: UI bridge via WebSocket

To let the MCP server signal the React UI (e.g. "open this file", "highlight this element"), add a WebSocket channel alongside the stdio MCP transport:

```
Claude Code ←stdio→ MCP Server ←WebSocket→ React Frontend
```

- MCP server connects as a WebSocket **client** to a bridge server your Rust backend starts on a fixed local port (e.g. 9711).
- The bridge relays messages to all connected WebSocket clients, including the React WebView.
- React connects to `ws://localhost:9711` on startup and handles `ui_action` messages.

This two-transport design keeps the stdio MCP protocol clean while still allowing the agent to drive the UI.

---

## Step 6 — Build the React Frontend

### Recommended state shape

```typescript
type AgentStatus = 'idle' | 'running' | 'error'

interface ToolInvocation {
  toolName: string
  toolId: string
  input?: string
  output?: string
  done: boolean
}

interface Message {
  role: 'user' | 'assistant'
  text: string        // accumulated TextDelta chunks
  thinking?: string   // accumulated ThinkingDelta chunks
  tools: ToolInvocation[]
}
```

### Streaming text accumulation

Append `TextDelta` chunks to the last assistant message as they arrive — do not wait for `Result`:

```typescript
case 'TextDelta':
  setMessages(prev => {
    const last = prev[prev.length - 1]
    if (last?.role === 'assistant') {
      return [...prev.slice(0, -1), { ...last, text: last.text + data.text }]
    }
    return [...prev, { role: 'assistant', text: data.text, tools: [] }]
  })
  break
```

### Tool call display

Show a card for each tool call between `ToolStart` and `ToolDone`. The pattern:

```typescript
case 'ToolStart':
  setTools(prev => new Map(prev).set(data.tool_id, {
    toolName: data.tool_name,
    toolId: data.tool_id,
    input: data.input,
    done: false,
  }))
  break

case 'ToolDone':
  setTools(prev => {
    const next = new Map(prev)
    const existing = next.get(data.tool_id)
    if (existing) next.set(data.tool_id, { ...existing, output: data.output, done: true })
    return next
  })
  break
```

### Session persistence for multi-turn conversations

Store the `session_id` from the `Init` event and pass it back as `--resume <session_id>` on subsequent messages. This lets Claude remember context across turns without you managing conversation history yourself.

```typescript
case 'Init':
  sessionIdRef.current = data.session_id
  break

// When sending the next message:
invoke('stream_agent', {
  request: {
    message: userText,
    session_id: sessionIdRef.current,  // null on first message
    vault_path: projectPath,
  }
})
```

---

## Gotchas and Edge Cases

### Authentication errors

Check stderr for auth-related strings when the process exits with a non-zero code:

```rust
fn is_auth_error(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    ["not logged in", "authentication", "auth"].iter().any(|s| lower.contains(s))
}
```

Surface a clear message: `"Claude CLI is not authenticated. Run 'claude auth login' in your terminal."`

### Nested session guard

The environment variable `CLAUDECODE` is set by Claude Code when it is running. A second invocation will see this and refuse to start ("nested session"). Always call `.env_remove("CLAUDECODE")` before spawning.

### Event listener leak

Each call to `listen('agent-stream', ...)` registers a new handler. If you forget to call the returned `unlisten()` function, old handlers accumulate. Always call `unlisten()` in `finally`.

### Concurrent requests

Tauri events are broadcast to all listeners. If the user sends a second message while the first is still running, both streams will fire on the same `'agent-stream'` channel and their events will interleave. Options:
- Disable the input while a request is in flight.
- Add a request ID to each event and filter client-side.
- Use separate event names per request.

Tolaria takes the simplest approach: disable input while `status === 'running'`.

### Binary not found on CI or sandboxed environments

In test environments where the CLI is not installed, `find_claude_binary()` will return an error. Design your UI to show a setup prompt rather than crashing — check for the binary on startup and gate the AI feature behind that check.

### `--output-format stream-json` requires `--verbose`

Without `--verbose`, Claude Code may not emit all intermediate stream events. Always pair them together.

### Large tool outputs

Tool results can be large (e.g. file contents). Don't render the raw output string directly in the UI — truncate to a preview and offer an expand action.

---

## Minimal End-to-End Example

Below is the smallest possible implementation of the complete pattern.

### Rust (`src-tauri/src/main.rs`)

```rust
use tauri::Emitter;
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::process::Stdio;

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "kind")]
enum StreamEvent {
    TextDelta { text: String },
    Done,
    Error { message: String },
}

#[derive(Deserialize)]
struct AgentRequest {
    message: String,
    vault_path: String,
}

#[tauri::command]
async fn stream_agent(
    app_handle: tauri::AppHandle,
    request: AgentRequest,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let binary = std::path::PathBuf::from("claude"); // simplified: use find_claude_binary() in practice

        let mut child = std::process::Command::new(&binary)
            .args([
                "-p", &request.message,
                "--output-format", "stream-json",
                "--verbose",
                "--include-partial-messages",
                "--tools", "",
            ])
            .current_dir(&request.vault_path)
            .env_remove("CLAUDECODE")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn: {e}"))?;

        let stdout = child.stdout.take().ok_or("No stdout")?;
        for line in std::io::BufReader::new(stdout).lines() {
            let line = line.map_err(|e| format!("Read error: {e}"))?;
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else { continue };

            if json["type"] == "stream_event" {
                if let Some(text) = json["event"]["delta"]["text"].as_str() {
                    let _ = app_handle.emit("agent-stream", StreamEvent::TextDelta { text: text.into() });
                }
            }
        }

        let _ = app_handle.emit("agent-stream", StreamEvent::Done);
        child.wait().map_err(|e| format!("Wait failed: {e}"))?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {e}"))?
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![stream_agent])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
```

### TypeScript (`src/App.tsx`)

```typescript
import { useState, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

type StreamEvent = { kind: 'TextDelta'; text: string } | { kind: 'Done' } | { kind: 'Error'; message: string }

export function App() {
  const [input, setInput] = useState('')
  const [response, setResponse] = useState('')
  const [running, setRunning] = useState(false)

  async function send() {
    setRunning(true)
    setResponse('')

    const unlisten = await listen<StreamEvent>('agent-stream', (event) => {
      if (event.payload.kind === 'TextDelta') {
        setResponse(prev => prev + event.payload.text)
      }
    })

    try {
      await invoke('stream_agent', {
        request: { message: input, vault_path: '/path/to/project' }
      })
    } finally {
      unlisten()
      setRunning(false)
    }
  }

  return (
    <div>
      <textarea value={input} onChange={e => setInput(e.target.value)} />
      <button onClick={send} disabled={running}>Send</button>
      <pre>{response}</pre>
    </div>
  )
}
```

This minimal example handles text streaming only. Add the session ID, tool cards, thinking blocks, MCP config, and binary discovery from earlier sections to build out the full experience.
