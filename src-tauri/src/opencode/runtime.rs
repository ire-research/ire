//! Owns the single `opencode serve` process for the active IRE workspace,
//! started lazily on first use and torn down on workspace close.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::process::{Child, Command};
use tokio::sync::Mutex as AsyncMutex;

use crate::opencode::client::{OpenCodeClient, OpenCodeError};
use crate::opencode::events::{self, OpenCodeEvent, OpenCodeSessionState};
use crate::session::{RunningTurn, SessionManager};
use crate::stream_event::{AskQuestion, AskQuestionOption, StreamEvent};

/// Routing + per-turn bookkeeping for one OpenCode session, mapped to its
/// IRE tab. `stream_id`/`event_id` reset at the start of every new turn.
pub(crate) struct TabRoute {
    pub(crate) tab_id: String,
    pub(crate) stream_id: String,
    pub(crate) event_id: u64,
    pub(crate) state: OpenCodeSessionState,
}

pub(crate) struct RuntimeInner {
    pub(crate) workspace: PathBuf,
    pub(crate) client: OpenCodeClient,
    child: std::sync::Mutex<Child>,
    sse_task: tauri::async_runtime::JoinHandle<()>,
    pub(crate) sessions: Arc<AsyncMutex<HashMap<String, TabRoute>>>,
}

/// Tauri-managed state: at most one running OpenCode server, for the
/// current `ActiveWorkspace`.
#[derive(Default)]
pub struct OpenCodeRuntime(AsyncMutex<Option<Arc<RuntimeInner>>>);

impl OpenCodeRuntime {
    /// Returns the running server for `workspace`, starting one if needed.
    /// Stops a stale server for a different workspace first, defensively.
    pub async fn ensure_started(
        &self,
        app: &AppHandle,
        session_manager: &SessionManager,
        workspace: &Path,
        bin: &Path,
        mcp_config: Option<&Path>,
    ) -> Result<Arc<RuntimeInner>, OpenCodeError> {
        let mut guard = self.0.lock().await;
        if let Some(inner) = guard.as_ref() {
            if inner.workspace.as_path() == workspace {
                return Ok(Arc::clone(inner));
            }
            let stale = guard.take().unwrap();
            drop(guard);
            stop_inner(&stale).await;
            guard = self.0.lock().await;
        }

        let inner = Arc::new(
            start_process(app.clone(), session_manager.clone(), workspace, bin, mcp_config).await?,
        );
        *guard = Some(Arc::clone(&inner));
        Ok(inner)
    }

    /// The currently running server, if any — never starts one itself.
    pub async fn current(&self) -> Option<Arc<RuntimeInner>> {
        self.0.lock().await.clone()
    }

    /// Aborts every known session, then kills the server process.
    pub async fn stop(&self) {
        let inner = self.0.lock().await.take();
        if let Some(inner) = inner {
            stop_inner(&inner).await;
        }
    }
}

async fn stop_inner(inner: &Arc<RuntimeInner>) {
    inner.sse_task.abort();
    let session_ids: Vec<String> = inner.sessions.lock().await.keys().cloned().collect();
    for session_id in session_ids {
        let _ = inner.client.abort_session(&session_id).await;
    }
    if let Ok(mut child) = inner.child.lock() {
        let _ = child.start_kill();
    }
}

async fn start_process(
    app: AppHandle,
    session_manager: SessionManager,
    workspace: &Path,
    bin: &Path,
    mcp_config: Option<&Path>,
) -> Result<RuntimeInner, OpenCodeError> {
    let config = crate::opencode::config::server_config(mcp_config);

    let mut cmd = Command::new(bin);
    cmd.arg("serve")
        .arg("--hostname")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("0")
        .current_dir(workspace)
        .env("OPENCODE_CONFIG_CONTENT", config)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| OpenCodeError(format!("failed to spawn opencode serve: {e}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| OpenCodeError("no stdout from opencode serve".to_string()))?;

    let base_url = read_listening_url(stdout).await.ok_or_else(|| {
        let _ = child.start_kill();
        OpenCodeError("opencode serve did not report a listening address".to_string())
    })?;

    let client = OpenCodeClient::new(base_url.clone());
    if let Err(e) = wait_for_health(&client).await {
        let _ = child.start_kill();
        return Err(e);
    }

    let sessions: Arc<AsyncMutex<HashMap<String, TabRoute>>> = Arc::new(AsyncMutex::new(HashMap::new()));
    let sse_task = {
        let sessions = Arc::clone(&sessions);
        let client = client.clone();
        let base_url = base_url.clone();
        tauri::async_runtime::spawn(async move {
            run_sse_loop(app, client, base_url, sessions, session_manager).await;
        })
    };

    Ok(RuntimeInner {
        workspace: workspace.to_path_buf(),
        client,
        child: std::sync::Mutex::new(child),
        sse_task,
        sessions,
    })
}

/// `opencode serve` prints one plain "listening on http://HOST:PORT" line
/// to stdout once bound; that's the only stdout output to expect.
async fn read_listening_url(stdout: tokio::process::ChildStdout) -> Option<String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut lines = BufReader::new(stdout).lines();
    let read = async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(url) = line.strip_prefix("opencode server listening on ") {
                return Some(url.trim().to_string());
            }
        }
        None
    };
    tokio::time::timeout(Duration::from_secs(15), read).await.ok().flatten()
}

async fn wait_for_health(client: &OpenCodeClient) -> Result<(), OpenCodeError> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if client.health().await {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(OpenCodeError(
                "opencode serve did not become healthy in time".to_string(),
            ));
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

/// Maintains one `/event` SSE subscription for the server's whole lifetime,
/// reconnecting with bounded backoff. Only registered tab routes are acted on.
async fn run_sse_loop(
    app: AppHandle,
    client: OpenCodeClient,
    base_url: String,
    sessions: Arc<AsyncMutex<HashMap<String, TabRoute>>>,
    session_manager: SessionManager,
) {
    let http = reqwest::Client::new();
    let mut backoff = Duration::from_millis(500);
    loop {
        match http.get(format!("{base_url}/event")).send().await {
            Ok(resp) => {
                backoff = Duration::from_millis(500);
                let mut buf = String::new();
                let mut stream = resp.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    let Ok(bytes) = chunk else { break };
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = buf.find("\n\n") {
                        let frame: String = buf.drain(..pos + 2).collect();
                        for line in frame.lines() {
                            let Some(data) = line.strip_prefix("data:") else { continue };
                            let Ok(raw) = serde_json::from_str::<Value>(data.trim_start()) else {
                                continue;
                            };
                            handle_event(&app, &client, &sessions, &session_manager, &raw).await;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "opencode /event connection failed, retrying");
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(10));
    }
}

/// No `permission.asked` handling: the config overlay sets `permission:
/// "allow"` unconditionally, so OpenCode never emits that event.
async fn handle_event(
    app: &AppHandle,
    client: &OpenCodeClient,
    sessions: &Arc<AsyncMutex<HashMap<String, TabRoute>>>,
    session_manager: &SessionManager,
    raw: &Value,
) {
    match events::parse_event(raw) {
        OpenCodeEvent::MessageUpdated { session_id, message_id, role } => {
            let mut guard = sessions.lock().await;
            if let Some(route) = guard.get_mut(&session_id) {
                if role == "assistant" {
                    route.state.mark_assistant_message(message_id);
                } else {
                    route.state.mark_other_message(message_id);
                }
            }
        }
        OpenCodeEvent::MessagePartUpdated { session_id, part } => {
            let Some(message_id) = part["messageID"].as_str().map(str::to_string) else { return };
            resolve_message_role(client, sessions, &session_id, &message_id).await;

            let mut guard = sessions.lock().await;
            let Some(route) = guard.get_mut(&session_id) else { return };
            let tab_id = route.tab_id.clone();
            let stream_id = route.stream_id.clone();
            let mut pending = Vec::new();
            events::normalize_part(&part, &mut route.state, &mut |e| pending.push(e));
            for event in pending {
                route.event_id += 1;
                emit_stream(app, &tab_id, &stream_id, route.event_id, &event);
            }
        }
        OpenCodeEvent::SessionIdle { session_id } => {
            let mut guard = sessions.lock().await;
            let Some(route) = guard.get_mut(&session_id) else { return };
            let tab_id = route.tab_id.clone();
            let stream_id = route.stream_id.clone();
            route.event_id += 1;
            emit_stream(
                app,
                &tab_id,
                &stream_id,
                route.event_id,
                &StreamEvent::Result { text: None, session_id: session_id.clone() },
            );
            route.event_id += 1;
            emit_stream(app, &tab_id, &stream_id, route.event_id, &StreamEvent::Done);
            drop(guard);
            session_manager.clear_running_if(&tab_id, &RunningTurn::OpenCode { session_id });
        }
        OpenCodeEvent::SessionError { session_id: Some(session_id), message } => {
            let mut guard = sessions.lock().await;
            let Some(route) = guard.get_mut(&session_id) else { return };
            let tab_id = route.tab_id.clone();
            let stream_id = route.stream_id.clone();
            tracing::warn!(tab_id = %tab_id, error = %message, "opencode session error");
            route.event_id += 1;
            emit_stream(
                app,
                &tab_id,
                &stream_id,
                route.event_id,
                &StreamEvent::Error { message },
            );
            route.event_id += 1;
            emit_stream(app, &tab_id, &stream_id, route.event_id, &StreamEvent::Done);
            drop(guard);
            session_manager.clear_running_if(&tab_id, &RunningTurn::OpenCode { session_id });
        }
        OpenCodeEvent::SessionError { session_id: None, message } => {
            tracing::warn!(error = %message, "opencode session error with no session id");
        }
        OpenCodeEvent::QuestionAsked { session_id, request_id, questions } => {
            let mut guard = sessions.lock().await;
            let Some(route) = guard.get_mut(&session_id) else { return };
            let tab_id = route.tab_id.clone();
            let stream_id = route.stream_id.clone();
            route.event_id += 1;
            let event_id = route.event_id;
            drop(guard);
            session_manager.set_pending_opencode_question(&tab_id, request_id.clone());
            emit_stream(
                app,
                &tab_id,
                &stream_id,
                event_id,
                &StreamEvent::AskUserQuestion {
                    tool_id: request_id,
                    questions: parse_ask_questions(&questions),
                },
            );
        }
        OpenCodeEvent::Other => {}
    }
}

/// Resolves and caches a message id's role via `GET` if not already known —
/// doesn't rely on `message.updated` having arrived first.
async fn resolve_message_role(
    client: &OpenCodeClient,
    sessions: &Arc<AsyncMutex<HashMap<String, TabRoute>>>,
    session_id: &str,
    message_id: &str,
) {
    {
        let guard = sessions.lock().await;
        match guard.get(session_id) {
            Some(route) if route.state.role_known(message_id) => return,
            Some(_) => {}
            None => return,
        }
    }
    // A failed lookup leaves the id unresolved (retried on the next part
    // update) rather than caching a possibly-wrong negative result.
    let Ok(role) = client.get_message_role(session_id, message_id).await else { return };
    let mut guard = sessions.lock().await;
    if let Some(route) = guard.get_mut(session_id) {
        if role == "assistant" {
            route.state.mark_assistant_message(message_id.to_string());
        } else {
            route.state.mark_other_message(message_id.to_string());
        }
    }
}

fn parse_ask_questions(raw: &[Value]) -> Vec<AskQuestion> {
    raw.iter()
        .map(|q| AskQuestion {
            header: q["header"].as_str().unwrap_or_default().to_string(),
            question: q["question"].as_str().unwrap_or_default().to_string(),
            multi_select: q["multiple"].as_bool().unwrap_or(false),
            options: q["options"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|o| AskQuestionOption {
                    label: o["label"].as_str().unwrap_or_default().to_string(),
                    description: o["description"].as_str().map(str::to_string),
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn emit_stream(app: &AppHandle, tab_id: &str, stream_id: &str, event_id: u64, event: &StreamEvent) {
    let _ = app.emit(
        "chat-stream",
        serde_json::json!({
            "tab_id": tab_id,
            "stream_id": stream_id,
            "event_id": event_id,
            "event": event,
        }),
    );
}
