# Implementation Blueprint: Claude Code UI Wrapper (Streaming + Animations)

## 1. Technical Stack & Requirements

**Core Dependencies:**
- `tauri` (v2) — IPC bridge between Rust backend and web frontend (`tauri::Emitter`, `tauri::AppHandle`)
- `tokio` — async runtime for non-blocking subprocess management (`tokio::task::spawn_blocking`)
- `serde` / `serde_json` — JSON event parsing from Claude's NDJSON stdout
- `@tauri-apps/api` — `invoke()` (trigger command) + `listen()` (receive streamed events)
- `react` — component state driven by streamed events
- `phosphor-react` — icon library for tool-specific icons
- CSS keyframes / Tailwind `animate-spin` — streaming and tool-pending animations

**Environmental Prerequisites:**
- `claude` CLI binary installed and discoverable on `$PATH` or in known locations
- `CLAUDECODE` env var must be **removed** from subprocess environment to prevent nested session guard
- MCP config JSON file path required for agent mode (vault tools)

---

## 2. Architecture Map

| Layer | File | Role |
|---|---|---|
| Process spawn | `src-tauri/src/claude_cli.rs` | Build args, spawn `claude` subprocess, parse NDJSON lines |
| JSON line runner | `src-tauri/src/cli_agent_runtime.rs` | Generic line-by-line NDJSON consumer |
| Event types | `src-tauri/src/claude_cli.rs` (top) | `ClaudeStreamEvent` enum (`#[serde(tag = "kind")]`) |
| Tauri IPC | `src-tauri/src/commands/ai.rs` | `stream_ai_agent` Tauri command — emit events to frontend |
| Agent router | `src-tauri/src/ai_agents.rs` | Maps `AiAgentId` → correct CLI runner |
| Frontend bridge | `src/utils/streamAiAgent.ts` | `listen()` + `invoke()` orchestration |
| Callback contracts | `src/lib/aiAgentStreamCallbacks.ts` | Maps events → React `setMessages` mutations |
| Message state | `src/lib/aiAgentMessageState.ts` | Pure updater functions for message objects |
| Hook | `src/hooks/useCliAiAgent.ts` | Wires runtime state, exposes `sendMessage` |
| UI — message | `src/components/AiMessage.tsx` | Renders reasoning, tools, response, streaming indicator |
| UI — tool card | `src/components/AiActionCard.tsx` | Status icons, tool-specific icons, expandable I/O |
| Animations | `src/index.css` | `typing-bounce` keyframes, `ai-spin` class |

---

## 3. Step-by-Step Reproduction

### Step 1 — Define the Event Enum (Rust)

Define a serializable enum with `#[serde(tag = "kind")]` so each variant becomes a discriminated union on the TypeScript side:

```rust
// src-tauri/src/claude_cli.rs
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "kind")]
pub enum ClaudeStreamEvent {
    Init          { session_id: String },
    TextDelta     { text: String },
    ThinkingDelta { text: String },
    ToolStart     { tool_name: String, tool_id: String, input: Option<String> },
    ToolDone      { tool_id: String, output: Option<String> },
    Result        { text: String, session_id: String },
    Error         { message: String },
    Done,
}
```

Mirror this exactly as a TypeScript discriminated union (add `kind` as the discriminant).

---

### Step 2 — Spawn the Subprocess and Stream NDJSON (Rust)

Claude CLI supports `--output-format stream-json --verbose --include-partial-messages`. Each output line is a self-contained JSON object.

```rust
// Generic NDJSON runner pattern (cli_agent_runtime.rs)
pub(crate) fn run_json_line_process<Event, F, H>(
    mut command: Command,
    emit: &mut F,
    mut handle_json: H,
) -> Result<(), String>
where
    F: FnMut(Event),
    H: FnMut(&serde_json::Value, &mut F),
{
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().ok_or("no stdout")?;
    for line in BufReader::new(stdout).lines() {
        if let Ok(line) = line {
            if let Ok(json) = serde_json::from_str::<Value>(&line) {
                handle_json(&json, emit);
            }
        }
    }
    child.wait()?;
    Ok(())
}
```

**Critical CLI args to pass:**
```rust
fn build_agent_args(req: &AgentStreamRequest) -> Vec<String> {
    vec![
        "--output-format".into(), "stream-json".into(),
        "--verbose".into(),
        "--include-partial-messages".into(),
        "--permission-mode".into(), "acceptEdits".into(),
        "--mcp-config".into(), mcp_config_path,
        "-p".into(), req.message.clone(),
    ]
}
```

Remove `CLAUDECODE` from env to prevent nested session detection:
```rust
cmd.env_remove("CLAUDECODE");
```

---

### Step 3 — Parse JSON Lines into Typed Events (Rust)

Dispatch on the `type` (and nested `subtype`) fields Claude emits:

```rust
fn dispatch_event<F: FnMut(ClaudeStreamEvent)>(
    json: &Value,
    state: &mut StreamState,
    emit: &mut F,
) {
    match json["type"].as_str().unwrap_or("") {
        "system" if json["subtype"].as_str() == Some("init") => {
            let sid = json["session_id"].as_str().unwrap_or("").to_string();
            emit(ClaudeStreamEvent::Init { session_id: sid });
        }
        "stream_event" => dispatch_stream_event(json, state, emit),
        "result" => {
            emit(ClaudeStreamEvent::Result {
                text: extract_result_text(json),
                session_id: state.session_id.clone(),
            });
            emit(ClaudeStreamEvent::Done);
        }
        _ => {}
    }
}

fn dispatch_stream_event<F: FnMut(ClaudeStreamEvent)>(
    json: &Value,
    state: &mut StreamState,
    emit: &mut F,
) {
    let event = &json["stream_event"];
    match event["type"].as_str().unwrap_or("") {
        "content_block_start" => {
            if event["content_block"]["type"].as_str() == Some("tool_use") {
                let id   = event["content_block"]["id"].as_str().unwrap_or("").to_string();
                let name = event["content_block"]["name"].as_str().unwrap_or("").to_string();
                state.current_tool_id = Some(id.clone());
                emit(ClaudeStreamEvent::ToolStart { tool_name: name, tool_id: id, input: None });
            }
        }
        "content_block_delta" => {
            let delta = &event["delta"];
            match delta["type"].as_str() {
                Some("text_delta")     => emit(ClaudeStreamEvent::TextDelta     { text: delta["text"].as_str().unwrap_or("").to_string() }),
                Some("thinking_delta") => emit(ClaudeStreamEvent::ThinkingDelta { text: delta["thinking"].as_str().unwrap_or("").to_string() }),
                _ => {}
            }
        }
        "content_block_stop" => {
            if let Some(id) = state.current_tool_id.take() {
                emit(ClaudeStreamEvent::ToolDone { tool_id: id, output: None });
            }
        }
        _ => {}
    }
}
```

---

### Step 4 — Bridge to Frontend via Tauri IPC (Rust)

Use `tokio::task::spawn_blocking` to run the synchronous subprocess loop off the async executor. Emit each event via `app_handle.emit()`:

```rust
// src-tauri/src/commands/ai.rs
#[tauri::command]
pub async fn stream_ai_agent(
    app_handle: tauri::AppHandle,
    request: AiAgentStreamRequest,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        run_normalized_ai_agent_stream(request, Box::new(move |event| {
            let _ = app_handle.emit("ai-agent-stream", &event);
        }))
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}
```

Register with `tauri::Builder::invoke_handler(tauri::generate_handler![stream_ai_agent])`.

---

### Step 5 — Subscribe and Dispatch on the Frontend (TypeScript)

```typescript
// src/utils/streamAiAgent.ts
export async function streamAiAgent(request: StreamAiAgentRequest): Promise<void> {
    const { invoke } = await import('@tauri-apps/api/core')
    const { listen }  = await import('@tauri-apps/api/event')

    // Subscribe BEFORE invoking — avoid race condition with early events
    const unlisten = await listen<AiAgentStreamEvent>('ai-agent-stream', (tauriEvent) => {
        const payload = tauriEvent.payload
        if (payload.kind === 'Done') { unlisten(); return }
        handleStreamEvent(payload, callbacks)
    })

    try {
        await invoke<string>('stream_ai_agent', { request })
    } catch (err) {
        callbacks.onError(String(err))
    } finally {
        unlisten()
    }
}

function handleStreamEvent(data: AiAgentStreamEvent, cb: AgentStreamCallbacks): void {
    switch (data.kind) {
        case 'TextDelta':     cb.onText(data.text);                                       break
        case 'ThinkingDelta': cb.onThinking(data.text);                                   break
        case 'ToolStart':     cb.onToolStart(data.tool_name, data.tool_id, data.input);   break
        case 'ToolDone':      cb.onToolDone(data.tool_id, data.output);                   break
        case 'Error':         cb.onError(data.message);                                   break
        case 'Done':          cb.onDone();                                                break
    }
}
```

---

### Step 6 — Accumulate State in React (Callbacks → `setMessages`)

```typescript
// src/lib/aiAgentStreamCallbacks.ts
export function createStreamCallbacks(ctx: StreamMutationContext): AgentStreamCallbacks {
    const { setMessages, messageId, setStatus, responseAccRef } = ctx

    return {
        onThinking: (chunk) =>
            updateMessage(setMessages, messageId, (m) => ({
                ...m, reasoning: (m.reasoning ?? '') + chunk,
            })),

        onText: (chunk) => {
            markReasoningDone(setMessages, messageId)
            responseAccRef.current += chunk   // ref accumulator — not setState per token
        },

        onToolStart: (toolName, toolId, input) => {
            setStatus('tool-executing')
            updateMessage(setMessages, messageId, (m) =>
                appendToolAction(m, { tool: toolName, toolId, status: 'pending', input })
            )
        },

        onToolDone: (toolId, output) =>
            updateMessage(setMessages, messageId, (m) => ({
                ...m,
                actions: m.actions.map((a) =>
                    a.toolId === toolId ? { ...a, status: 'done', output } : a
                ),
            })),

        onDone: () => {
            setStatus('done')
            updateMessage(setMessages, messageId, (m) => ({
                ...m,
                isStreaming: false,
                response: responseAccRef.current,
                reasoningDone: true,
            }))
        },

        onError: (message) => {
            setStatus('error')
            updateMessage(setMessages, messageId, (m) => ({ ...m, isStreaming: false, error: message }))
        },
    }
}
```

---

### Step 7 — Render the Streaming Message (React)

```tsx
// src/components/AiMessage.tsx
function ConversationMessage({
    reasoning, reasoningDone, actions, response, isStreaming,
}: AiMessageProps) {
    return (
        <div>
            <UserBubble content={userMessage} />

            {/* Expandable thinking block — auto-scrolls to bottom while streaming */}
            {reasoning && <ReasoningBlock text={reasoning} isDone={reasoningDone} />}

            {/* One card per ToolStart event */}
            {actions.map((a) => <AiActionCard key={a.toolId} {...a} />)}

            {/* Final response text */}
            {response && <ResponseBlock text={response} />}

            {/* 3-dot typing animation — visible only before first TextDelta */}
            {isStreaming && !response && <StreamingIndicator />}
        </div>
    )
}

function ReasoningBlock({ text, isDone }: { text: string; isDone: boolean }) {
    const contentRef = useRef<HTMLDivElement>(null)

    useEffect(() => {
        if (!isDone && contentRef.current) {
            contentRef.current.scrollTop = contentRef.current.scrollHeight
        }
    }, [text, isDone])

    return (
        <div>
            <button onClick={toggle}><Brain size={14} /> Reasoning</button>
            {expanded && (
                <div ref={contentRef} style={{ maxHeight: 200, overflowY: 'auto' }}>
                    {text}
                </div>
            )}
        </div>
    )
}

function StreamingIndicator() {
    return (
        <div className="flex items-center gap-1">
            <span className="typing-dot" />
            <span className="typing-dot" />
            <span className="typing-dot" />
        </div>
    )
}
```

---

### Step 8 — Tool Action Card with Status Animations

```tsx
// src/components/AiActionCard.tsx
function StatusIndicator({ status }: { status: 'pending' | 'done' | 'error' }) {
    if (status === 'pending') return <CircleNotch size={14} className="animate-spin text-muted-foreground" />
    if (status === 'done')    return <CheckCircle size={14} weight="fill" style={{ color: 'var(--accent-green)' }} />
    return                           <XCircle     size={14} weight="fill" style={{ color: 'var(--destructive)' }} />
}

const TOOL_ICON: Record<string, (size: number) => ReactNode> = {
    Bash:         (s) => <Terminal        size={s} />,
    Write:        (s) => <PencilSimple    size={s} />,
    Edit:         (s) => <NotePencil      size={s} />,
    Read:         (s) => <File            size={s} />,
    search_notes: (s) => <MagnifyingGlass size={s} />,
    get_note:     (s) => <File            size={s} />,
    open_note:    (s) => <Eye             size={s} />,
}
```

---

### Step 9 — CSS Animations

```css
/* src/index.css */

/* Typing indicator (3 bouncing dots shown while waiting for first token) */
.typing-dot {
    display: inline-block;
    width: 6px; height: 6px;
    border-radius: 50%;
    background: var(--muted-foreground);
    animation: typing-bounce 1.2s ease-in-out infinite;
}
.typing-dot:nth-child(2) { animation-delay: 0.2s; }
.typing-dot:nth-child(3) { animation-delay: 0.4s; }

@keyframes typing-bounce {
    0%, 60%, 100% { opacity: 0.3; transform: translateY(0); }
    30%            { opacity: 1;   transform: translateY(-3px); }
}

/* Tool-executing spinner */
.ai-spin {
    animation: spin 1s linear infinite;
}
@keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
}
```

---

## 4. Critical Logic Constraints

**Subscribe before invoke.** The `listen()` call must resolve before `invoke()` fires the Tauri command. The Rust side starts emitting immediately; a late subscription silently drops leading events (first tokens, `Init`).

**Remove `CLAUDECODE` from subprocess env.** Claude CLI sets this env var to guard against nested sessions. If it propagates to the child process, the CLI refuses to start.

**Use a ref accumulator for text deltas.** Calling `setMessages` on every `TextDelta` (potentially hundreds per second) causes excessive re-renders. Buffer into a `useRef` string and flush to React state only on `onDone` or a debounced tick.

**Reasoning auto-scroll.** The reasoning block must auto-scroll to the bottom as new thinking chunks arrive. Use a `useEffect` that fires on `text` change and sets `scrollTop = scrollHeight` while `isDone` is false.

**`spawn_blocking` for subprocess I/O.** The stdout BufReader is synchronous. Wrapping it in `tokio::task::spawn_blocking` prevents it from starving the Tauri async executor and keeps the UI responsive during long agent runs.

**`#[serde(tag = "kind")]` on the Rust enum.** Without this, Serde wraps each variant in a keyed object (`{"TextDelta": {"text": "..."}}`) rather than a flat discriminated union (`{"kind": "TextDelta", "text": "..."}`). The flat form maps cleanly to TypeScript's `switch (data.kind)` pattern.

**Tool `status: 'pending'` → `'done'` transition drives the spinner.** The card renders a `CircleNotch` (spinner) while `status === 'pending'` and a `CheckCircle` on `'done'`. This visual transition is entirely data-driven — no imperative animation code needed.

**`isStreaming: true` flag controls the typing dots.** Append the message to state with `isStreaming: true` immediately on send. `StreamingIndicator` renders only when `isStreaming && !response`. Once the first `TextDelta` lands and is flushed, the dots disappear naturally.
