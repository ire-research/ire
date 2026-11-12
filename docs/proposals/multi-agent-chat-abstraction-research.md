# Multi-agent chat abstraction: prior art (research)

Research note, not a design proposal — no code change is implied. It surveys how
other projects that orchestrate one or more coding-agent backends abstract "one turn
of chat, streamed back," and where they put session/resume-id persistence, to inform
the decision in `AgentProvider`/`CliTurn` (`src-tauri/src/agent_provider.rs`) on
whether resume-id persistence should become a trait method or stay a free function
each provider calls at its own trigger point.

All claims below are sourced from primary repo files (fetched directly from GitHub)
or official protocol docs, not blog posts or marketing pages. Where a finding relies
on a secondary summary rather than a directly-read file, that is flagged.

## 1. Zed's Agent Client Protocol (ACP)

Zed is the closest architectural analog to `ire`: a Rust desktop app driving multiple
agent backends — its own native agent, plus Claude Code, Gemini CLI, and Codex as
external subprocesses — via the **Agent Client Protocol (ACP)**, a JSON-RPC-over-stdio
protocol Zed co-designed specifically to standardize "editor talks to heterogeneous
coding-agent subprocesses"
([agentclientprotocol.com](https://agentclientprotocol.com),
[github.com/agentclientprotocol/agent-client-protocol](https://github.com/agentclientprotocol/agent-client-protocol)).

**The protocol itself separates session lifecycle from turn-sending as distinct
JSON-RPC methods**
([`agent-client-protocol-schema/src/v2/agent.rs`](https://github.com/agentclientprotocol/agent-client-protocol/blob/main/agent-client-protocol-schema/src/v2/agent.rs)):
`session/new` and `session/resume` mint or resume a session and return the session id
directly in the response (not via a later stream event); `session/prompt` sends a
message on an already-established session and streams `session/update` notifications
back. `ResumeSessionRequest`'s doc comment: "Resumes an existing session and
optionally replays prior conversation history according to `replayFrom`."

**Zed's own client-side Rust trait, however, puts session lifecycle and turn-sending
on the same trait.** `AgentConnection`
([`crates/acp_thread/src/connection.rs`](https://github.com/zed-industries/zed/blob/main/crates/acp_thread/src/connection.rs),
verified directly against source, lines ~91-202):

```rust
pub trait AgentConnection {
    fn new_session(self: Rc<Self>, project: Entity<Project>, work_dirs: PathList, cx: &mut App)
        -> Task<Result<Entity<AcpThread>>>;

    fn supports_load_session(&self) -> bool { false }
    fn load_session(self: Rc<Self>, session_id: acp::SessionId, ...) -> Task<Result<Entity<AcpThread>>> { .. }

    fn supports_resume_session(&self) -> bool { false }
    fn resume_session(self: Rc<Self>, session_id: acp::SessionId, ...) -> Task<Result<Entity<AcpThread>>> { .. }

    fn supports_close_session(&self) -> bool { false }
    fn close_session(self: Rc<Self>, session_id: &acp::SessionId, cx: &mut App) -> Task<Result<()>> { .. }

    fn prompt(&self, params: acp::PromptRequest, cx: &mut App) -> Task<Result<acp::PromptResponse>>;
    fn cancel(&self, session_id: &acp::SessionId, cx: &mut App);
}
```

`new_session`/`load_session`/`resume_session`/`close_session` and `prompt`/`cancel`
are all methods of **one trait**. Backends that can't support load/resume/close don't
implement those methods at all — they inherit the default `supports_*() -> false` /
error-returning implementations, so the trait doesn't force every backend into the
same calling convention; it forces them to *declare* what they can do, with the
default absorbing the rest. This is a directly relevant precedent for the
`AgentProvider`/`CliTurn` split `ire` already has (an optional capability trait layered
onto a base trait) — Zed uses the same layering shape, just folded into one trait with
default methods instead of two traits.

There's a separate lower-level trait, `AgentServer`
([`crates/agent_servers/src/agent_servers.rs`](https://github.com/zed-industries/zed/blob/main/crates/agent_servers/src/agent_servers.rs)),
that only handles *launching* a backend (`connect()`, `logo()`, `agent_id()`) and has
no session-id method at all — it hands off a connection object that implements
`AgentConnection` above. So the "no persistence on this trait" observation applies to
the launcher trait, not to the trait that actually runs turns.

**Durable storage of the session id — surviving app restarts, not just held in
memory — is a separate concern from `AgentConnection` itself.** Per a secondary
summary of `crates/agent/src/thread_store.rs` (not independently verified against raw
file content in this research pass — flagged as secondary-source), a `ThreadStore`
persists threads to a `ThreadsDatabase`, and a `Session` struct
(`crates/agent/src/agent.rs`) pairs a durable `Thread` entity with its live
`AcpThread` protocol handle. So the full picture is layered: the protocol-level
lifecycle trait (`AgentConnection`) is centralized alongside turn-sending, but the
actual disk write is a shared store called from that trait's implementations, not a
method on the trait itself.

## 2. opencode's own server

Relevant on two counts: it's the shape `ire`'s own OpenCode provider already talks to,
and its internal session module is a second real-world data point on the same
question. Verified directly against
[sst/opencode](https://github.com/sst/opencode) (`dev` branch,
`packages/core/src/session/` and `packages/opencode/src/server/`):

- `POST /session` (`session.create`) returns `Session.Info` — including the id —
  **synchronously in the HTTP response**, before any message is sent. This is exactly
  `ire`'s own OpenCode adapter pattern: create session, persist its id, then send the
  turn — no stream to hook into, because the id exists before the turn starts.
- `POST /session/:sessionID/message` (`session.prompt`) sends one turn; a *separate*,
  single, workspace-scoped `GET /event` endpoint (SSE, `event.subscribe`) carries live
  updates for every session the client cares about — opencode does not open one stream
  per prompt call.
- Internally, [`session/store.ts`](https://github.com/sst/opencode/blob/dev/packages/core/src/session/store.ts)
  (read/persist), [`session/prompt.ts`](https://github.com/sst/opencode/blob/dev/packages/core/src/session/prompt.ts)
  + `session/runner/` (turn execution, provider-agnostic across opencode's many LLM
  backends), and `session.ts`/`session/info.ts` (id-minting) are three separate
  modules — not one interface wearing all three hats. (A newer, event-sourced rewrite
  of opencode under a renamed org, `anomalyco/opencode`, keeps the same separation:
  `Session.Interface.prompt()` durably records the prompt as an event and wakes the
  runner; `events()` is a distinct streaming method reading the durable event log by
  cursor, and `SessionStore.Service` is a distinct persistence service. Session ids
  there are minted synchronously at `create()`, before the `SessionV1.Event.Created`
  event is even published, so there's no case of "learn the id mid-stream" at all —
  the id predates any run.)

opencode's answer to "when do you persist the session id" is unambiguous and matches
`ire`'s own OpenCode provider: synchronously, before the turn starts. It has no
stream-derived-id pattern anywhere in its own architecture.

## 3. VS Code Chat Participant API / Copilot Chat

VS Code takes a third approach for its built-in chat participant model: **the host
(VS Code) owns session continuity entirely**, and the extension never persists a
resume id itself. A participant registers one `ChatRequestHandler` via
`createChatParticipant(name, handler)`
([vscode-extension-samples/chat-sample](https://github.com/microsoft/vscode-extension-samples/blob/main/chat-sample/src/simple.ts));
VS Code calls it with `(request, context, stream, token)`, where `request.sessionId`
and `context.history` are supplied by the host on every call, replaying prior turns
rather than the extension resuming an opaque backend token. There is no
session-persistence method on the participant interface at all, because there's
nothing for the extension to persist — this is architecturally different from CLI
subprocess agents (Claude Code, Codex) that hand back an opaque resume token the host
must store.

Copilot Chat's newer `chatSessionsProvider` proposed API
([`vscode.proposed.chatSessionsProvider.d.ts`](https://github.com/microsoft/vscode-copilot-chat/blob/main/src/extension/vscode.proposed.chatSessionsProvider.d.ts))
is the closer analog to `ire`'s problem — it's how Copilot Chat surfaces external
agent CLIs (Codex, Claude Code) as sessions inside VS Code's UI. There, a
`ChatSession` object bundles `history`, a `requestHandler`, and an
`activeResponseCallback` on one object — turn execution and the session's identity
(`resource: Uri`) live together — but *how* the provider maps that session back to
the underlying CLI's own resume token is left entirely to the provider's own
implementation, invisible to this interface. VS Code doesn't standardize that
persistence trigger point at all; each provider does it its own way, wherever it
needs to.

## 4. Aider

Single-backend (talks to LLM APIs directly, no external agent CLI to resume), so
weaker prior art for `ire`'s specific problem, but its internal shape is consistent
with the others. In
[`aider/coders/base_coder.py`](https://github.com/Aider-AI/aider/blob/main/aider/coders/base_coder.py),
`send_message(inp)` is the one-turn-of-chat method: appends to `cur_messages`, calls
`send()` (which streams from the model), applies edits. History persistence to
`.aider.chat.history.md` happens outside `send_message()`, triggered after the turn
completes rather than folded into the send call itself. (Continue.dev has an
analogous core-vs-IDE message protocol but was not independently verified with
primary source in this pass.)

## Pattern across all sources

| Project | Turn-send interface | Session id: when known | Session id: where persisted |
|---|---|---|---|
| Zed / ACP (protocol) | `session/prompt` RPC | synchronously, in `session/new`/`session/resume` response | client-side, not specified by protocol |
| Zed (Rust client) | `AgentConnection::prompt` | synchronously, in `new_session`/`load_session`/`resume_session` return value — same trait as `prompt` | separate `ThreadStore`/`ThreadsDatabase`, called from the trait impl (secondary-sourced) |
| opencode (both codebases) | `session.prompt` / `Session.Interface.prompt()` | synchronously, at `session.create` — always before any turn | `session/store.ts` / `SessionStore.Service` — separate module, called synchronously at creation |
| VS Code (built-in participants) | `ChatRequestHandler` | n/a — host replays history, no backend resume id | host-owned; extension does nothing |
| VS Code (Copilot Chat external agents) | `ChatSession.requestHandler` | unstandardized — provider's own concern | provider-owned, unstandardized |
| Aider | `send_message()` | n/a (stateless API calls) | `.aider.chat.history.md`, written after the turn, outside `send_message()` |

Two findings matter most for `ire`:

1. **No project examined persists a resume/session id off a mid-stream event.**
   Every backend that has an id at all (Zed/ACP, opencode) knows it synchronously,
   before or at the start of the turn — because in their designs, session creation is
   always a distinct, prior step from prompting, never folded into the first
   streamed chunk of a turn. `ire`'s two CLI providers are the odd case here: Claude
   Code and Codex only reveal the resume id *as part of* the first turn's own output
   stream (there is no separate "create session" call before the first message), so
   there is no clean prior-art precedent for "persist off a stream event" — it's a
   consequence of those two CLIs' own designs, not of any orchestration layer's
   choice.
2. **Whether lifecycle and turn-sending share a trait/interface varies, but the
   closest analog (Zed's `AgentConnection`) puts them on the same trait**, using
   capability-flag defaults so backends that can't do load/resume aren't forced to
   implement it. opencode, by contrast, keeps them as separate modules/services
   throughout both its codebases (old and new), even though session creation is
   always synchronous there. VS Code's built-in model doesn't have adapter-owned
   persistence at all; its external-agent model (`chatSessionsProvider`) leaves it
   fully unstandardized.

## Relevance to ire's `AgentProvider` seam

`ire`'s situation combines pieces of both patterns: like opencode, one provider
(OpenCode) knows its session id synchronously before the turn starts; like Claude
Code/Codex's own CLI design (not really like any orchestrator surveyed here), the
other two only reveal it mid-stream, because that's how those CLIs' own protocols
work — there's no separate "create session" round-trip available to call synchronously
instead.

Given that, the strongest, most directly-verified precedent — Zed's `AgentConnection`
trait — supports putting resume-id persistence **behind a trait method**, not a bare
free function, but doing it the way Zed does: a thin trait method (default
no-op/false, overridden per backend) that each implementation calls at its *own*
natural trigger point — inside the stream-dispatch loop for Claude/Codex, right after
`session.create` for OpenCode — while the actual disk/DB write stays a single shared
internal helper (mirroring Zed's separate `ThreadStore` and opencode's separate
`session/store.ts`, both called synchronously from within the higher-level
call site rather than duplicated).

Concretely, this means: add something like `fn record_resume_id(&self, session_id:
&str, resume_id: &str)` as a default-provided method on `AgentProvider` (or a small
capability trait alongside `CliTurn`, following the shape `ire` already uses for
provider-specific capabilities), with a single shared free function or module doing
the actual write underneath it. Claude/Codex call it from their `StreamEvent::Init`
handling; OpenCode calls it synchronously after `session.create`, before the prompt is
sent. This centralizes the *call site* — no more duplicating the write logic three
times — without forcing all three providers into one uniform calling convention, and
it matches the one piece of prior art (Zed) that was built to solve exactly this
problem and was directly verified against source rather than inferred. A pure free
function with no trait surface at all would also work and is simpler to write today,
but it forgoes the one benefit Zed's design demonstrates: a trait method makes "does
this provider record a resume id, and how" a discoverable, overridable part of the
provider's declared interface rather than an implicit convention callers have to
remember to invoke correctly at three different call sites.
