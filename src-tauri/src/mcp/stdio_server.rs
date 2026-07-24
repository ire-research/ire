//! Rust stdio MCP server exposed via `ire --mcp-stdio`.
//!
//! Replaces the former Node `mcp/server.js`. It is a thin stdio MCP front end
//! that forwards every tool call to the running app over `IRE_BACKEND_SOCKET`
//! (the same Unix-socket RPC the Node bridge used). All real work happens in
//! [`crate::mcp::rpc`]; this process only advertises the catalog and relays.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, JsonObject, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use serde_json::{json, Value};

/// Entry point for `ire --mcp-stdio`. Runs until stdin closes.
pub fn run_stdio() {
    let socket_path = match std::env::var("IRE_BACKEND_SOCKET") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("IRE_BACKEND_SOCKET not set");
            std::process::exit(1);
        }
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    rt.block_on(async move {
        let service = IreMcp { socket_path }
            .serve((tokio::io::stdin(), tokio::io::stdout()))
            .await
            .expect("serve mcp stdio");
        let _ = service.waiting().await;
    });
}

#[derive(Clone)]
struct IreMcp {
    socket_path: String,
}

impl ServerHandler for IreMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::LATEST;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::new("ire", "1.0.0");
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(tool_catalog()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        if request.name.as_ref() == "ask_user_question" && ask_excluded() {
            return Err(McpError::invalid_params(
                "ask_user_question is not available for this agent — use the built-in question tool instead",
                None,
            ));
        }

        let params = request
            .arguments
            .map(Value::Object)
            .unwrap_or_else(|| json!({}));

        match self.call_backend(&request.name, params).await {
            Ok(result) => {
                let text = match result {
                    Value::String(s) => s,
                    other => serde_json::to_string_pretty(&other)
                        .unwrap_or_else(|_| other.to_string()),
                };
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }
}

impl IreMcp {
    #[cfg(unix)]
    async fn call_backend(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let stream = UnixStream::connect(&self.socket_path).await?;
        let (read_half, mut write_half) = stream.into_split();

        let mut bytes = serde_json::to_vec(&json!({
            "id": 1,
            "method": method,
            "params": params,
        }))?;
        bytes.push(b'\n');
        write_half.write_all(&bytes).await?;

        let mut line = String::new();
        BufReader::new(read_half).read_line(&mut line).await?;

        let resp: Value = serde_json::from_str(line.trim())?;
        if resp["ok"].as_bool().unwrap_or(false) {
            Ok(resp.get("result").cloned().unwrap_or_else(|| json!({})))
        } else {
            let err = resp["error"].as_str().unwrap_or("backend error");
            Err(anyhow::anyhow!(err.to_string()))
        }
    }

    #[cfg(not(unix))]
    async fn call_backend(&self, _method: &str, _params: Value) -> anyhow::Result<Value> {
        anyhow::bail!("MCP backend bridge is only implemented on Unix")
    }
}

fn schema(value: Value) -> Arc<JsonObject> {
    Arc::new(value.as_object().cloned().unwrap_or_default())
}

fn tool(name: &'static str, description: &'static str, input_schema: Value) -> Tool {
    Tool::new(name, description, schema(input_schema))
}

/// Set only on the MCP subprocess(es) an OpenCode server spawns (see
/// `opencode::config::server_config`): OpenCode has its own native
/// `question` tool, and letting an OpenCode turn see both would be
/// ambiguous about which one to use — see
/// docs/opencode-server-integration.md "Native questions, not IRE's MCP
/// question tool". Claude and Codex never set this and keep seeing
/// `ask_user_question` unchanged.
fn ask_excluded() -> bool {
    std::env::var_os("IRE_MCP_EXCLUDE_ASK").is_some()
}

/// Catalog mirrored from the former `mcp/server.js`. Descriptions and schemas
/// are the single source of truth agents see via `tools/list`.
fn tool_catalog() -> Vec<Tool> {
    let mut tools = vec![
        tool(
            "ire.read",
            "Read the workspace's ire.json (notes, focus, ideas, experiments). Returns its raw JSON `content` and a `version` token. You MUST call this before ire.edit.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "ire.edit",
            "Edit ire.json by exact string replacement (like the built-in Edit tool): replace `old` with `new`. Requires the `version` from a prior ire.read; fails if the version is stale (the file changed) or if `old` is missing or not unique. The result must remain valid against the ire.json schema.",
            json!({
                "type": "object",
                "properties": {
                    "old": { "type": "string", "description": "Exact substring to replace (must be unique in ire.json)" },
                    "new": { "type": "string", "description": "Replacement text" },
                    "version": { "type": "string", "description": "The version token returned by the latest ire.read" }
                },
                "required": ["old", "new", "version"]
            }),
        ),
        tool(
            "resource.add",
            "Add a research resource directly from markdown you supply (no fetching). Opens an Approve/Discard preview tab in the UI; on Approve it is written to resources/<slug>.md. The markdown should be a complete wiki file with frontmatter (title, TL;DR, etc.); IRE injects the `sources` you pass into the frontmatter.",
            json!({
                "type": "object",
                "properties": {
                    "markdown": { "type": "string", "description": "Full markdown body of the resource (ideally with frontmatter)" },
                    "title": { "type": "string", "description": "Optional human-readable title" },
                    "sources": {
                        "type": "array",
                        "description": "Optional source references (URLs or file paths) to record in frontmatter",
                        "items": { "type": "string" }
                    }
                },
                "required": ["markdown"]
            }),
        ),
        tool(
            "memory.write_long_term",
            "Append an entry to long-term memory (long-term.md) under a named section. Use for architectural decisions, pivots, durable insights, and dead ends worth preserving.",
            json!({
                "type": "object",
                "properties": {
                    "section": { "type": "string", "description": "Section heading (e.g. \"Architecture Decision\")" },
                    "content": { "type": "string", "description": "Content to append under the section" }
                },
                "required": ["section", "content"]
            }),
        ),
        tool(
            "memory.write_short_term",
            "Append to today's short-term memory (short-term/YYYY-MM-DD.md). Use for daily operational notes, debugging steps, experiment details, and transient dead ends.",
            json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Content to append to today's notes" }
                },
                "required": ["content"]
            }),
        ),
        tool(
            "ask_user_question",
            "Ask the user one or more multiple-choice questions and block until they respond in the IRE UI. Use this instead of guessing when you need the user to pick between options.",
            json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "description": "Questions to ask, shown together as a form.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "header": { "type": "string", "description": "Short label for this question (e.g. \"Approach\")" },
                                "question": { "type": "string", "description": "The question text" },
                                "multiSelect": { "type": "boolean", "description": "Allow selecting multiple options (default false)" },
                                "options": {
                                    "type": "array",
                                    "description": "Choices the user can pick from",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "label": { "type": "string", "description": "Option label" },
                                            "description": { "type": "string", "description": "Optional explanation of this option" }
                                        },
                                        "required": ["label"]
                                    },
                                    "minItems": 1
                                }
                            },
                            "required": ["header", "question", "options"]
                        },
                        "minItems": 1
                    }
                },
                "required": ["questions"]
            }),
        ),
        tool(
            "experiment.start",
            "Spawn a detached experiment subprocess. Returns immediately with a uuid. IRE will wake you up via --resume when the process finishes.",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Short human-readable experiment name" },
                    "command": { "type": "string", "description": "Shell command to run (passed to sh -c)" },
                    "working_dir": { "type": "string", "description": "Working directory (defaults to workspace root)" },
                    "wake_prompt": { "type": "string", "description": "Prompt to send when the experiment finishes" }
                },
                "required": ["name", "command", "wake_prompt"]
            }),
        ),
        tool(
            "experiment.status",
            "Get the status of an experiment by uuid.",
            json!({
                "type": "object",
                "properties": {
                    "uuid": { "type": "string", "description": "Experiment uuid returned by experiment.start" }
                },
                "required": ["uuid"]
            }),
        ),
        tool(
            "experiment.tail_logs",
            "Return the tail of stdout and stderr logs for an experiment.",
            json!({
                "type": "object",
                "properties": {
                    "uuid": { "type": "string", "description": "Experiment uuid" },
                    "kb": { "type": "number", "description": "Kilobytes to tail (default 64)" }
                },
                "required": ["uuid"]
            }),
        ),
    ];
    if ask_excluded() {
        tools.retain(|t| t.name.as_ref() != "ask_user_question");
    }
    tools
}
