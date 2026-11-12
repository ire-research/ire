# Dynamic, LLM-Generated Interfaces for IRE

Research date: August 21, 2026

## Decision

Build IRE around declarative generative UI, not LLM-generated React or HTML that executes inside the main app. The model should create, update, pin, and remove pages, panels, and workflow widgets only by composing trusted components, data queries, and actions.

This is now a recognized technical category, Generative UI (GenUI): the model selects layout and components for a task and session context; the host retains authority over rendering, data access, and execution.

The closest direct precedents are A2UI for the protocol and update model, Vercel json-render for the component-catalog implementation pattern, OpenUI/AppLess for the whole-app vision, and MCP Apps for a sandboxed escape hatch for specialized panels.

No mature, production-proven open-source desktop application currently lets an LLM safely redesign its entire durable application shell while controlling privileged local data. Credible protocols, frameworks, products, and HCI evidence exist, but the fully dynamic application is still early-stage. That is an opportunity for IRE and a reason to preserve a non-generative core shell.

## The strongest systems to study

### A2UI — closest protocol fit

[A2UI (Agent-to-User Interface)](https://github.com/a2ui-project/a2ui) is an Apache-2.0, open-source protocol for an agent to emit updateable UI as a stream of declarative JSON objects. The renderer maps abstract component types to components owned by the client. The model sends data, not executable code.

~~~text
agent -> JSONL UI messages -> renderer -> trusted native React components
  ^                                                    |
  |---------------- typed user actions ----------------|
~~~

The protocol creates a named surface, incrementally updates its component tree and data model, and lets the renderer return actions. Its component catalog defines the only UI types the model may use. The same specification can render in Web, React, Flutter, or native clients. The v1.0 specification is a candidate and the project is still evolving, so treat it as a strong reference and optional compatibility target, not a dependency IRE cannot move without.

Why it fits IRE:

- It treats an interface as a long-lived surface, not a chat attachment.
- It supports patches and a synchronized data model. A researcher can ask to add a loss curve above an experiment table without regenerating a page.
- It is designed for host-owned design systems and cross-trust-boundary rendering.
- It defines a catalog, ordered streaming, capability negotiation, and action return paths.

What not to copy blindly:

- IRE does not need cross-vendor transport on day one. A smaller IRE UI IR is faster to evolve, as long as it preserves A2UI-like concepts.
- A2UI is a UI description protocol, not an authorization system or data runtime.
- Its React ecosystem is recent. Pin versions and never accept an unrecognized catalog from an agent.

Sources: [project overview](https://github.com/a2ui-project/a2ui), [v1 protocol](https://github.com/a2ui-project/a2ui/blob/main/specification/v1_0/docs/a2ui_protocol.md), [renderer ecosystem](https://github.com/a2ui-project/a2ui/blob/main/docs/public/ecosystem/renderers.md).

### json-render — closest implementation fit

[Vercel Labs json-render](https://github.com/vercel-labs/json-render) is Apache-2.0 and implements the same core idea in TypeScript. The app defines a schema and a catalog of component props, actions, and functions. It generates the LLM prompt from that catalog. The model returns a flat JSON element tree; the renderer validates it and maps it only to registered components.

It already supports React, a flat streaming-friendly tree, actions, data binding, conditional visibility, and Zustand adapters. Its examples include a generated dashboard with charts and real-time data binding. That is unusually close to IRE's current React/Vite/Zustand shape.

The reusable principle is the split between grammar and vocabulary:

- The schema determines how a UI tree is encoded.
- The catalog determines which components, prop types, and actions exist.
- The registry maps catalog names to real React components.

It is the best codebase to study or use for a one-week spike. It remains 0.x web-framework software, so it must not become the owner of IRE permissions, database access, or persistent workspace state.

Sources: [README](https://github.com/vercel-labs/json-render/blob/main/README.md), [catalog model](https://json-render.dev/docs/catalog), [spec format](https://json-render.dev/docs/specs), [examples](https://json-render.dev/examples).

### OpenUI and AppLess — closest whole-app experiment

[OpenUI](https://github.com/thesysdev/openui) is an open-source full-stack GenUI framework. It uses a compact streaming language rather than JSON, generates model instructions from a component contract, and renders through a trusted component library. Its architecture makes the production-critical distinction: generate a UI once, then let the runtime execute queries, mutations, and reactive state without an LLM round trip for every click.

[AppLess](https://github.com/thesysdev/appless) is the clearest public proof of the dynamic-application idea. Each screen is generated when requested and rendered as native Cupertino or Material UI. One component contract produces the system prompt and two design-system renderers. Its public README explicitly says actions are simulated and real integrations are not wired yet. It is a visionary reference, not evidence that local-data authorization is solved.

Copy these three ideas:

- Generate prompt instructions from the UI contract so prompt and renderer cannot silently diverge.
- Generate once; execute direct. Filtering or refreshing must use a trusted query, not regenerate UI through an LLM.
- Allow incremental UI edits that preserve bindings and local state.

Sources: [OpenUI repository](https://github.com/thesysdev/openui), [OpenUI execution architecture](https://www.openui.com/docs/openui-lang/how-it-works), [AppLess repository](https://github.com/thesysdev/appless).

### MCP Apps — portable rich panels, not the whole shell

[MCP Apps](https://modelcontextprotocol.io/extensions/apps/overview) is now an official MCP extension. An MCP tool can return a developer-authored interactive HTML app inline in a conversational host. The host renders it in a sandboxed iframe, with a constrained JSON-RPC bridge; the embedded app can request tool calls through the host. The official client matrix lists shipped support in ChatGPT, Claude, VS Code GitHub Copilot, Goose, Postman, and MCPJam.

MCP Apps should not be the default renderer for IRE's own dynamic pages: an iframe is visually separate from the native app, and an agent would still need to create a code bundle. It is the correct later escape hatch for third-party research plugins, specialized visualizations or simulators, interactive PDF/sheet review, or a reviewable mini-app that should also work in other MCP hosts.

Sources: [MCP Apps overview](https://modelcontextprotocol.io/extensions/apps/overview), [official launch](https://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/), [SDK](https://apps.extensions.modelcontextprotocol.io/).

### OpenAI Apps SDK — production product precedent

The [OpenAI Apps SDK](https://help.openai.com/en/articles/12515353-build-with-the-apps-sdk) lets developers define tool logic and interactive UI that runs inside ChatGPT. Natural language can select an app, which appears in the conversation. Its examples include interactive flows returned alongside structured tool output. This work has converged toward MCP Apps: an MCP server, declared UI resource, and isolated widget bridge.

It proves conversational selection of an interactive interface is already a real product model. It does not mean ChatGPT lets an LLM rewrite its navigation or shell. The widget is developer-authored and scoped to a conversation.

Sources: [Apps SDK overview](https://help.openai.com/en/articles/12515353-build-with-the-apps-sdk), [examples](https://github.com/openai/openai-apps-sdk-examples), [launch](https://openai.com/index/introducing-apps-in-chatgpt/).

### AG-UI, CopilotKit, and assistant-ui

[AG-UI](https://docs.copilotkit.ai/ag-ui) is an event protocol between agent backends and frontends. It streams agent state, tool calls, shared state, interrupts, and generative UI. It is complementary to A2UI: AG-UI is a transport/lifecycle layer; A2UI is a UI description format.

[assistant-ui's Generative UI example](https://www.assistant-ui.com/examples/generative-ui) shows a useful product pattern: one present capability exposes a component vocabulary, so a model composes a dashboard without each component being attached to a specific tool. It also keeps this generic route separate from known tool-specific renderers. Apply that idea to IRE's chat: do not make generic present_ui compete with a specialized show_experiment_log route unless both are actually desired.

### Adaptive Cards — the important predecessor

[Adaptive Cards](https://learn.microsoft.com/en-us/microsoft-copilot-studio/adaptive-cards-overview) are a mature precursor: JSON UI snippets render natively in a host and collect typed user input. Microsoft Copilot Studio uses them for agent responses. They are templated and constrained rather than free composition of the whole experience.

Copy the precedent, not the library: host-owned visual language, declared inputs/actions, versioned schemas, and channel compatibility. It demonstrates why user-visible UI must remain readable and predictable even when an agent selects its content.

## What is not the same thing

| Category | Examples | Why it differs |
| --- | --- | --- |
| AI app builders | v0, Lovable, Bolt, Figma Make | The model edits source code for a separate app. Build/preview/deploy is the loop, not a trusted runtime. |
| Document/code canvases | ChatGPT Canvas, Gemini Canvas, Claude Artifacts | The assistant creates an artifact in a fixed host surface. The host UI is not dynamically reconfigured. |
| Tool-specific cards | most AI SDK tool UIs, static Copilot cards | Useful for known tools, but the developer predetermines every renderer. |
| Raw-code runtime demos | generated HTML/JSX, Renderify | Maximum freedom, but arbitrary code, XSS, capability leakage, versioning, and accessibility are the price. |

IRE should eventually support generated apps only as a reviewed, sandboxed artifact workflow. It should never use them as the authority boundary for its core workspace UI.

## Research evidence and design lessons

### DynaVis (CHI 2024, Best Paper)

[DynaVis](https://arxiv.org/abs/2401.10880) lets a person request a chart edit in natural language, performs it, and creates a persistent widget for further edits. In a 24-participant study, users preferred it to a natural-language-only UI, citing immediate feedback and easier refinement.

The direct lesson for IRE: generate persistent, modular controls after an intent is understood. Do not make a researcher repeat a prompt to change a plot series, resource filter, or experiment view. Let the model create a dedicated editable control surface and keep it until the user removes it.

DynaVis generated HTML and JavaScript callbacks. Copy the modularity, persistence, declarative state, and compositional transforms; do not copy runtime arbitrary code execution.

### Graphologue (UIST 2023)

[Graphologue](https://arxiv.org/abs/2305.11473) turns LLM responses into interactive diagrams in real time. Users manipulate a graph and submit context-specific follow-up prompts. The contribution is not a total generated UI; it shows that a graphical, non-linear view can make complex LLM output easier to explore than a linear transcript.

For IRE, research plans, literature relationships, experiment genealogy, and argument maps are excellent built-in dynamic surface types. They should remain queryable and editable, not be screenshots in chat.

### GenerativeGUI and Generative Interfaces for Language Models

[GenerativeGUI](https://researchr.org/publication/HojoSYSSNS25) specifically investigates dynamic GUI generation tailored to the ongoing conversation. It is a direct academic precedent, though an extended abstract rather than a full system blueprint.

[Generative Interfaces for Language Models](https://arxiv.org/abs/2508.19227) creates structured interface representations and iteratively refines them from queries. Its human evaluation reports preference for generated interfaces over conversational ones in more than 70% of cases. The paper also cautions that its HTML/JavaScript system has no backend logic, can take minutes to refine, and generates UI even when it is unnecessary.

The IRE conclusion is not “always generate an interface.” Add a should_present_surface decision: use chat for explanation and small answers; create a surface only when it gives persistent control, dense comparison, direct manipulation, or continuing monitoring value.

### Open-ended Generative UI research

[Generative UI: LLMs are Effective UI Generators](https://generativeui.github.io/static/pdfs/paper.pdf) evaluates full HTML/JavaScript generation. It reports high preference over markdown in its setting, while also reporting one-to-two-minute generation and occasional JavaScript/CSS/HTML errors. It is evidence that bespoke interfaces can be valuable, not evidence that raw generated code is safe inside a local-first research desktop app.

### Design governance

[Canvil](https://explore.openaire.eu/search/result?pid=10.1145%2F3706598.3713139), a CHI 2025 paper, found that designers need a way for design requirements and LLM behavior to shape each other. IRE must expose a small human-authored UI contract: visual tokens, component descriptions, layout limits, and policies. Without this grammar, dynamic interface drifts into visually inconsistent, opaque, hard-to-debug software.

## Recommended IRE architecture

### Keep the shell stable

Never allow generation to replace:

- window controls, workspace identity, settings, update path, safety/approval modals;
- chat transcript and composer;
- audit trail, undo/restore, and return-to-default-layout;
- the authority boundary for local files, processes, credentials, experiments, and MCP servers.

Everything else can be a named dynamic surface: center tab, right-rail view, left-rail section, inspector, or popover. This maps directly to IRE's existing resizable left rail, center pane, and right rail.

### Make a versioned UI contract

Create one source of truth that generates TypeScript validation and the agent-facing tool schema/prompt reference.

- Layout: Stack, Grid, Split, Tabs, Section, Card.
- Content: Text, Markdown, Metric, Table, KeyValue, Badge, Progress, Code, CitationList.
- Research widgets: ExperimentTable, ExperimentLog, MetricChart, ResourceList, PaperReader, WikiFile, Diff, ResearchGraph, TaskList.
- Inputs: TextField, Select, MultiSelect, Toggle, Slider, DateRange, Form, ConfirmButton.
- Actions: refresh, navigate, send_chat_message, open_resource, open_experiment, request_approval, and narrowly typed domain mutations.
- Data sources: named read queries and named mutations, each with input, output, freshness, and approval metadata.
- Hard limits: valid placements, tree depth, node count, message size, refresh rate, action count, and chart-data limits.

Never allow generated class names, inline CSS, browser JavaScript, direct Tauri commands, raw SQL, arbitrary file paths, or raw shell commands. The model chooses composition and trusted wiring, not executable behavior or authority.

### Use a flat, patchable surface specification

A flat node map is better than nested JSX for streaming, validation, and targeted edits:

~~~json
{
  "version": 1,
  "surface": { "id": "exp-overview", "slot": "center-tab", "title": "Ablation overview" },
  "root": "page",
  "nodes": {
    "page": { "type": "Stack", "props": { "gap": "md" }, "children": ["title", "runs"] },
    "title": { "type": "Heading", "props": { "text": "Learning-rate ablation" } },
    "runs": {
      "type": "ExperimentTable",
      "props": { "query": "experiments.list", "columns": ["name", "status", "best_val_loss"] },
      "actions": { "rowClick": { "type": "open_experiment", "idFromRow": "uuid" } }
    }
  }
}
~~~

The runtime validates every node against the catalog and replaces failures with a readable error card. Later add upsert_node, remove_node, set_surface, and set_data operations. This preserves scroll position, form values, expanded rows, and cached query data when the user asks for a small change.

### Generate once; execute directly

~~~text
user request
  -> agent inspects permitted workspace data
  -> ui.present creates a declarative surface
  -> React renderer mounts trusted components
  -> user filters/sorts/edits
  -> action broker invokes a named Rust query or mutation
  -> workspace events refresh bound widgets
~~~

Changing an experiment-status filter should call experiments.list with a status, not ask an LLM to redraw a table. The model returns only for semantic redesign: group by hypothesis, make a failure triage page, or turn this into a weekly report.

### Make UI a native IRE MCP capability

IRE already normalizes external-agent tool streams. Add a native, agent-callable capability:

~~~text
ire.ui.present(surface_spec)
ire.ui.patch(surface_id, operations)
ire.ui.close(surface_id)
ire.ui.list_catalog()
~~~

Do not parse magic markdown fences from assistant prose. An explicit tool call gives a typed request, visible tool card, approval point, failure reason, and provider-independent path for Claude Code, Codex, and OpenCode.

The Rust side should validate schema/catalog/bindings/quotas, run placement and permission policy, persist accepted revisions plus audit metadata, emit a Tauri event, and broker every later action against a user/session/permission context. The frontend renders accepted revisions and can own local presentation state; it must not decide authorization.

### Persist user-owned surfaces, not model ephemera

Store surfaces as workspace state with IDs, revisions, creator agent/model, source conversation/turn, pinned status, and a bounded semantic-request snapshot. Preserve user direct changes separately from agent proposals. Provide pin/unpin, duplicate, rename, restore revision, reset shell, and “Why this view?” with the originating prompt and explanation.

Do not persist every transient suggestion. Require pinning or expire a surface with its chat/tab.

## Security is architecture, not a sanitizer

IRE combines private local files, agents, shell/process execution, MCP connections, and untrusted papers/web content. A compromised prompt must not gain a more powerful UI route than it already has.

> The model may propose a view. Only the native policy/action broker may read, write, execute, or authorize.

Apply these controls:

1. Declarative default. Render known components only. Never evaluate generated JSX, JS, HTML handlers, CSS URLs, or imports in the IRE webview.
2. Capability-scoped actions. An action is an ID plus typed parameters, never a command string. Rust revalidates it for workspace, user, resource, and confirmation policy every time.
3. Immutable approval. A generated view can request an action but cannot render a fake approval that authorizes it. Use a native host modal with exact parameters and short expiry.
4. Read-data catalog. Panels bind only to purpose-specific read queries. Do not expose arbitrary file search, SQL, network fetch, process output, or cross-workspace state reactively.
5. Untrusted-content isolation. Papers, web pages, tool responses, MCP descriptions, and agent outputs are data, never instructions. They cannot modify catalog, policy, prompt, or stored surface without an explicit workflow.
6. Separate custom code. For a future arbitrary tool UI, use MCP Apps-style sandboxing or a separate zero-capability Tauri webview. Never give it Tauri invoke; provide only a narrow postMessage/MCP bridge.
7. Audit and replay. Log proposals, validation rejects, catalog versions, accepted action parameters, confirmations, and source turn. Redact secrets and private document text.
8. Resource limits. Enforce component depth/node limits, query/mutation rate limits, maximum surface count, patch size, and cancellation.

Tauri's capability system limits which frontend webviews can call which APIs, and remote content does not get access by default. However, registered Tauri commands are broadly callable unless explicitly restricted. Do not assume a sandbox makes a broad command safe. See [Tauri capabilities](https://v2.tauri.app/security/capabilities/).

This aligns with [MCP security principles](https://modelcontextprotocol.io/specification/2025-03-26/index), [OWASP MCP security](https://cheatsheetseries.owasp.org/cheatsheets/MCP_Security_Cheat_Sheet.html), and [OWASP agent security](https://cheatsheetseries.owasp.org/cheatsheets/AI_Agent_Security_Cheat_Sheet.html): enforce authorization downstream rather than trusting model output.

## Rollout plan

### Phase 0 — contract spike (one week)

Build no autonomy. Add a hand-authored SurfaceSpec, component registry, schema validation, and generated-center-tab renderer. Render four static fixtures: experiment table, resource dashboard, chart + filter, research graph. Verify persistence, restore, light/dark styling, keyboard navigation, and invalid-schema error cards.

Success: IRE safely renders and restores a declarative page with no model in the loop.

### Phase 1 — agent-created surfaces (one to two weeks)

Expose read-only ire.ui.present through the native MCP bridge. Limit it to one new center tab and one inspector-style right-rail surface. Supply the catalog and exemplar specs. Initially confirm “show this proposed view” before placement.

Use six real tasks:

- Show failed experiments grouped by failure cause.
- Make an ablation dashboard for this week.
- Create a page that compares these three papers.
- Give me a live panel for running experiments and last log lines.
- Turn hypotheses in this chat into an editable decision board.
- Create a reading queue from resources not summarized.

Success: every surface uses registered components and named read queries; users can pin, close, restore, and explain it.

### Phase 2 — direct manipulation and live bindings

Add local input state, query cache/invalidation, workspace-event subscriptions, and safe mutations. Classify each mutation: read-only, reversible, confirmation-required, or prohibited. Add patches so small agent edits preserve surface identity and local state.

Success: normal interaction has zero model round trips.

### Phase 3 — dynamic composition in rails and workspaces

Allow proposed left/right sections, split views, and pinned workspace dashboards. Keep top bar, chat, resize controls, and approval UI fixed. Add user rules such as “never auto-open panels” and “generated surfaces only in center.”

### Phase 4 — controlled escape hatch

Support MCP Apps or a reviewed local mini-app only for workloads the catalog cannot express: simulation, special domain editor, or heavyweight visualization. It is an unprivileged, auditable plugin with a declared bridge, never code generated and executed in one step.

## Evaluation plan

Do not evaluate only whether a surface parses. Evaluate whether researchers complete real tasks safely and faster.

- Contract correctness: unknown component/action, cyclic tree, invalid binding, over-limit data, unsupported version, and stale catalog fail closed.
- Action safety: generated UI cannot access commands, filesystem paths, secrets, network origins, or mutations outside approved action IDs and parameters.
- Replay tests: retain prompts, workspace snapshots, generated specs, expected actions, and expected semantic roles across providers/model versions.
- Task measures: completion, time, follow-up prompts, error recovery, data-provenance understanding, and recreation of prior views.
- Trust: users can identify what a widget reads, what an action does, and whether a result is live, stale, or proposed.
- Accessibility: keyboard-only flows, screen-reader labels, reduced-motion behavior, contrast, and no critical meaning in layout alone.
- Adversarial tests: injected PDF/web/MCP output attempts to create hidden actions, access unrelated data, fake approval, exhaust queries, or persist hostile instructions.

Test intent, interaction, and authorization, not exact pixels. Generated layouts are nondeterministic; the contract is that correct information and safe action affordances exist.

## Final recommendation

Use A2UI's protocol architecture, json-render's catalog/registry design, and OpenUI's generate-once/execute-directly rule. Implement a narrow, IRE-owned surface IR first. Deliver it to every agent through an explicit MCP tool. Keep privileged operations in Rust and the shell under user control.

Do not start with “the LLM freely writes the IRE frontend.” That creates a code-execution and usability problem, not a dynamic-interface system. A dynamic interface becomes useful when it is persistent, live, composable, explainable, reversible, and safely connected to real research data. That is the gap IRE can credibly fill.

