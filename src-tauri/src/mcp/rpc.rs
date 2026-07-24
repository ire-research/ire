use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Local;
use tauri::{AppHandle, Manager};

use crate::session::SessionManager;
use crate::commands::resources::{add_resource_from_markdown, InflightResources};
use crate::db::models as db;
use crate::ire::store::atomic_write;
use crate::ire::IreStore;

pub fn socket_path(home_data_dir: &Path) -> PathBuf {
    home_data_dir.join("mcp.sock")
}

/// Spawn the background Unix-socket RPC server. Returns a handle that can be
/// aborted to shut down the server.
#[cfg(unix)]
pub fn start(
    socket_path: PathBuf,
    workspace_root: PathBuf,
    session_manager: SessionManager,
    app: AppHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    let workspace_root = Arc::new(workspace_root);
    tauri::async_runtime::spawn(async move {
        run_server(socket_path, workspace_root, session_manager, app).await;
    })
}

#[cfg(not(unix))]
pub fn start(
    _socket_path: PathBuf,
    _workspace_root: PathBuf,
    _session_manager: SessionManager,
    _app: AppHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async {
        tracing::warn!("MCP RPC server is not implemented on this platform");
    })
}

#[cfg(unix)]
async fn run_server(
    socket_path: PathBuf,
    workspace_root: Arc<PathBuf>,
    session_manager: SessionManager,
    app: AppHandle,
) {
    use tokio::net::UnixListener;

    let _ = fs::remove_file(&socket_path);
    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, path = ?socket_path, "failed to bind MCP RPC socket");
            return;
        }
    };
    tracing::info!(path = ?socket_path, "MCP RPC server listening");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let workspace_root = Arc::clone(&workspace_root);
                let app = app.clone();
                let sm = session_manager.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, workspace_root, sm, app).await {
                        tracing::warn!(error = %e, "MCP RPC connection error");
                    }
                });
            }
            Err(e) => {
                tracing::debug!(error = %e, "MCP RPC accept error — shutting down");
                break;
            }
        }
    }
}

#[cfg(unix)]
async fn handle_connection(
    stream: tokio::net::UnixStream,
    workspace_root: Arc<PathBuf>,
    session_manager: SessionManager,
    app: AppHandle,
) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let req: serde_json::Value = serde_json::from_str(line.trim())?;
    let id = req["id"].clone();
    let method = req["method"].as_str().unwrap_or("").to_string();
    let params = req["params"].clone();

    let result = tokio::task::spawn_blocking(move || {
        dispatch(&method, &params, &workspace_root, &session_manager, &app)
    })
    .await
    .map_err(|e| anyhow!("task join error: {e}"))?;

    // A `dispatch` failure (missing active session, bad params, stale
    // version, ...) must still get a response line — propagating it via `?`
    // instead returns from this function before `write_half.write_all` runs,
    // which drops the connection with zero bytes written. The client then
    // sees a bare EOF instead of the actual error, surfacing as an opaque
    // "EOF while parsing a value" with no indication of what went wrong
    // (confirmed: this is the exact error OpenCode reported for a failed
    // `ask_user_question` call — the real failure reason was silently lost
    // here, not a problem in the tool-call dispatch itself).
    let resp = match result {
        Ok(value) => serde_json::json!({ "id": id, "ok": true, "result": value }),
        Err(e) => serde_json::json!({ "id": id, "ok": false, "error": e.to_string() }),
    };
    let mut bytes = serde_json::to_vec(&resp)?;
    bytes.push(b'\n');
    write_half.write_all(&bytes).await?;
    Ok(())
}

fn dispatch(
    method: &str,
    params: &serde_json::Value,
    workspace_root: &Path,
    session_manager: &SessionManager,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    tracing::debug!(method = %method, "mcp dispatch");
    let wiki = IreStore::new(workspace_root.to_path_buf());
    let result = match method {
        "ire.read" => ire_read(&wiki),
        "ire.edit" => ire_edit(params, &wiki, app),
        "memory.write_long_term" => memory_write_long_term(params, &wiki),
        "memory.write_short_term" => memory_write_short_term(params, &wiki),
        "resource.add" => resource_add(params, workspace_root, app),
        "ask_user_question" => ask_user_question(params, session_manager),
        "experiment.start" => experiment_start(params, workspace_root, session_manager, app),
        "experiment.status" => experiment_status(params, workspace_root),
        "experiment.tail_logs" => experiment_tail_logs(params, workspace_root),
        _ => Err(anyhow!("unknown method: {method}")),
    };
    match &result {
        Ok(_) => tracing::debug!(method = %method, "mcp dispatch ok"),
        Err(e) => tracing::warn!(method = %method, error = %e, "mcp dispatch failed"),
    }
    result
}

// ── ire.json ────────────────────────────────────────────────────────────────

fn ire_read(wiki: &IreStore) -> Result<serde_json::Value> {
    let (content, version) = wiki.read_ire_raw();
    Ok(serde_json::json!({ "content": content, "version": version }))
}

fn ire_edit(
    params: &serde_json::Value,
    wiki: &IreStore,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let old = params["old"].as_str().ok_or(anyhow!("missing old"))?;
    let new = params["new"].as_str().ok_or(anyhow!("missing new"))?;
    let version = params["version"]
        .as_str()
        .ok_or(anyhow!("missing version — call ire.read first"))?;
    wiki.edit_ire(old, new, version, app)?;
    Ok(serde_json::json!({ "edited": "ire.json" }))
}

// ── memory ──────────────────────────────────────────────────────────────────

fn memory_write_long_term(
    params: &serde_json::Value,
    wiki: &IreStore,
) -> Result<serde_json::Value> {
    let section = params["section"]
        .as_str()
        .ok_or(anyhow!("missing section"))?;
    let content = params["content"]
        .as_str()
        .ok_or(anyhow!("missing content"))?;

    let path = wiki.ire_dir.join("long-term.md");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let merged = format!("{existing}\n## {section}\n\n{content}\n");
    atomic_write(&path, &merged)?;
    Ok(serde_json::json!({ "written": "long-term.md" }))
}

fn memory_write_short_term(
    params: &serde_json::Value,
    wiki: &IreStore,
) -> Result<serde_json::Value> {
    let content = params["content"]
        .as_str()
        .ok_or(anyhow!("missing content"))?;

    let today = Local::now().format("%Y-%m-%d").to_string();
    let rel = format!("short-term/{today}.md");
    let path = wiki.ire_dir.join(&rel);
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let merged = format!("{existing}\n{content}\n");
    atomic_write(&path, &merged)?;
    Ok(serde_json::json!({ "written": rel }))
}

// ── resources ─────────────────────────────────────────────────────────────────

fn resource_add(
    params: &serde_json::Value,
    workspace_root: &Path,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let markdown = params["markdown"]
        .as_str()
        .ok_or(anyhow!("missing markdown"))?;
    let title = params["title"].as_str();
    let sources: Vec<String> = params["sources"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let inflight = app.state::<InflightResources>();
    let resource_id =
        add_resource_from_markdown(app, workspace_root, &inflight, markdown, title, &sources)?;
    Ok(serde_json::json!({ "resource_id": resource_id, "status": "pending_review" }))
}

/// Block until the user answers via `submit_ask_answer`. The CC subprocess
/// that issued this MCP call is the same one waiting on our response, so the
/// answer flows back as a normal tool_result — no session resume needed.
fn ask_user_question(
    params: &serde_json::Value,
    session_manager: &SessionManager,
) -> Result<serde_json::Value> {
    let active = session_manager
        .get_active_process_session()
        .ok_or_else(|| anyhow!("no active agent session"))?;

    let questions = params["questions"]
        .as_array()
        .cloned()
        .ok_or_else(|| anyhow!("missing questions"))?;

    let rx = session_manager.register_ask(&active.tab_id);
    let answers = rx
        .blocking_recv()
        .map_err(|_| anyhow!("question was cancelled before the user answered"))?;

    let out: Vec<serde_json::Value> = questions
        .iter()
        .enumerate()
        .map(|(i, q)| {
            serde_json::json!({
                "header": q["header"],
                "answer": answers.get(i).cloned().unwrap_or(serde_json::Value::Null),
            })
        })
        .collect();
    Ok(serde_json::json!({ "answers": out }))
}

// ── Experiment handlers ───────────────────────────────────────────────────────

fn experiment_start(
    params: &serde_json::Value,
    workspace_root: &Path,
    session_manager: &SessionManager,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let active = session_manager
        .get_active_session()
        .ok_or_else(|| anyhow!("no active agent session — cannot start experiment"))?;

    crate::experiments::runner::start_experiment(
        params,
        workspace_root,
        active,
        session_manager.clone(),
        app.clone(),
    )
}

fn experiment_status(
    params: &serde_json::Value,
    workspace_root: &Path,
) -> Result<serde_json::Value> {
    let uuid = params["uuid"]
        .as_str()
        .ok_or_else(|| anyhow!("missing uuid"))?;
    let home_data_dir = crate::workspace::init::home_data_dir(workspace_root)
        .ok_or_else(|| anyhow!("cannot determine home directory"))?;
    let row = db::get_experiment(&home_data_dir, uuid)?
        .ok_or_else(|| anyhow!("experiment {uuid} not found"))?;
    Ok(serde_json::json!({
        "uuid": row.uuid,
        "status": row.status,
        "exit_code": row.exit_code,
        "started_at": row.started_at,
        "ended_at": row.ended_at,
    }))
}

fn experiment_tail_logs(
    params: &serde_json::Value,
    workspace_root: &Path,
) -> Result<serde_json::Value> {
    let uuid = params["uuid"]
        .as_str()
        .ok_or_else(|| anyhow!("missing uuid"))?;
    let kb = params["kb"].as_u64().unwrap_or(64);
    let log_dir = workspace_root.join(".ire/cache/experiments").join(uuid);
    let max_bytes = kb * 1024;

    let stdout = tail_log(&log_dir.join("stdout.log"), max_bytes);
    let stderr = tail_log(&log_dir.join("stderr.log"), max_bytes);
    Ok(serde_json::json!({ "stdout": stdout, "stderr": stderr }))
}

fn tail_log(path: &Path, max_bytes: u64) -> String {
    let Ok(content) = fs::read(path) else {
        return String::new();
    };
    let len = content.len() as u64;
    let start = if len > max_bytes {
        (len - max_bytes) as usize
    } else {
        0
    };
    String::from_utf8_lossy(&content[start..]).into_owned()
}
