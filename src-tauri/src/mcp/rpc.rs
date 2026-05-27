use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Local;
use tauri::{AppHandle, Emitter};

use crate::cc::session::SessionManager;
use crate::commands::wiki::{patch_pulse_content, read_pulse_content, write_pulse_content};
use crate::db::models as db;
use crate::wiki::WikiStore;

pub fn socket_path(ire_dir: &Path) -> PathBuf {
    ire_dir.join("mcp.sock")
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
    .map_err(|e| anyhow!("task join error: {e}"))??;

    // If the result itself is an error object from dispatch, propagate it.
    let resp = if result.get("_rpc_error").is_some() {
        serde_json::json!({ "id": id, "ok": false, "error": result["_rpc_error"] })
    } else {
        serde_json::json!({ "id": id, "ok": true, "result": result })
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
    let wiki = WikiStore::new(workspace_root.to_path_buf());
    let result = match method {
        "wiki.read" => wiki_read(params, &wiki),
        "wiki.write" => wiki_write(params, &wiki, app),
        "wiki.append" => wiki_append(params, &wiki, app),
        "wiki.list" => wiki_list(&wiki),
        "wiki.rename" => wiki_rename(params, &wiki, app),
        "memory.write_long_term" => memory_write_long_term(params, &wiki, app),
        "memory.write_short_term" => memory_write_short_term(params, &wiki, app),
        "pulse.update" => pulse_update(params, &wiki, app),
        "experiment.start" => experiment_start(params, workspace_root, session_manager, app),
        "experiment.status" => experiment_status(params, workspace_root),
        "experiment.list" => experiment_list(params, workspace_root),
        "experiment.tail_logs" => experiment_tail_logs(params, workspace_root),
        _ => Err(anyhow!("unknown method: {method}")),
    };
    match &result {
        Ok(_) => tracing::debug!(method = %method, "mcp dispatch ok"),
        Err(e) => tracing::warn!(method = %method, error = %e, "mcp dispatch failed"),
    }
    result
}

fn wiki_read(params: &serde_json::Value, wiki: &WikiStore) -> Result<serde_json::Value> {
    let path = params["path"].as_str().ok_or(anyhow!("missing path"))?;
    let (content, frontmatter) = wiki.read(path)?;
    Ok(serde_json::json!({ "content": content, "frontmatter": frontmatter }))
}

fn wiki_write(
    params: &serde_json::Value,
    wiki: &WikiStore,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let path = params["path"].as_str().ok_or(anyhow!("missing path"))?;
    let content = params["content"]
        .as_str()
        .ok_or(anyhow!("missing content"))?;
    wiki.write(path, content, app)?;
    Ok(serde_json::json!({ "written": path }))
}

fn wiki_append(
    params: &serde_json::Value,
    wiki: &WikiStore,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let path = params["path"].as_str().ok_or(anyhow!("missing path"))?;
    let content = params["content"]
        .as_str()
        .ok_or(anyhow!("missing content"))?;
    let existing = fs::read_to_string(wiki.wiki_root.join(path)).unwrap_or_default();
    wiki.write(path, &(existing + content), app)?;
    Ok(serde_json::json!({ "appended": path }))
}

fn wiki_list(wiki: &WikiStore) -> Result<serde_json::Value> {
    let mut paths = Vec::new();
    collect_paths(&wiki.wiki_root, &wiki.wiki_root, &mut paths)?;
    paths.sort();
    Ok(serde_json::json!({ "paths": paths }))
}

fn collect_paths(wiki_root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_paths(wiki_root, &path, out)?;
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("md" | "json")
        ) {
            let rel = path
                .strip_prefix(wiki_root)?
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

fn wiki_rename(
    params: &serde_json::Value,
    wiki: &WikiStore,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let from = params["from"].as_str().ok_or(anyhow!("missing from"))?;
    let to = params["to"].as_str().ok_or(anyhow!("missing to"))?;
    wiki.rename(from, to, app)?;
    Ok(serde_json::json!({ "renamed": { "from": from, "to": to } }))
}

fn memory_write_long_term(
    params: &serde_json::Value,
    wiki: &WikiStore,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let section = params["section"]
        .as_str()
        .ok_or(anyhow!("missing section"))?;
    let content = params["content"]
        .as_str()
        .ok_or(anyhow!("missing content"))?;

    let path = "long-term.md";
    let existing = fs::read_to_string(wiki.wiki_root.join(path)).unwrap_or_default();
    let merged = format!("{existing}\n## {section}\n\n{content}\n");
    wiki.write(path, &merged, app)?;
    Ok(serde_json::json!({ "written": path }))
}

fn memory_write_short_term(
    params: &serde_json::Value,
    wiki: &WikiStore,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let content = params["content"]
        .as_str()
        .ok_or(anyhow!("missing content"))?;

    let today = Local::now().format("%Y-%m-%d").to_string();
    let path = format!("short-term/{today}.md");
    let existing = fs::read_to_string(wiki.wiki_root.join(&path)).unwrap_or_default();
    let merged = format!("{existing}\n{content}\n");
    wiki.write(&path, &merged, app)?;
    Ok(serde_json::json!({ "written": path }))
}

fn pulse_update(
    params: &serde_json::Value,
    wiki: &WikiStore,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let research_question = params["research_question"].as_str();
    let this_week = params["this_week"].as_str();

    let existing = read_pulse_content(wiki)?;
    let updated = patch_pulse_content(existing, research_question, this_week);
    write_pulse_content(wiki, &updated, app)?;

    let _ = app.emit(
        "pulse-changed",
        serde_json::json!({
            "research_question": updated.research_question,
            "this_week": updated.this_week,
        }),
    );

    Ok(serde_json::json!({ "updated": "pulse.json" }))
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
    let ire_dir = workspace_root.join(".ire");
    let row = db::get_experiment(&ire_dir, uuid)?
        .ok_or_else(|| anyhow!("experiment {uuid} not found"))?;
    Ok(serde_json::json!({
        "uuid": row.uuid,
        "status": row.status,
        "exit_code": row.exit_code,
        "started_at": row.started_at,
        "ended_at": row.ended_at,
    }))
}

fn experiment_list(params: &serde_json::Value, workspace_root: &Path) -> Result<serde_json::Value> {
    let limit = params["limit"].as_u64().unwrap_or(20) as usize;
    let ire_dir = workspace_root.join(".ire");
    let rows = db::list_experiments(&ire_dir, limit)?;
    Ok(serde_json::to_value(rows)?)
}

fn experiment_tail_logs(
    params: &serde_json::Value,
    workspace_root: &Path,
) -> Result<serde_json::Value> {
    let uuid = params["uuid"]
        .as_str()
        .ok_or_else(|| anyhow!("missing uuid"))?;
    let kb = params["kb"].as_u64().unwrap_or(64);
    let log_dir = workspace_root.join(".ire/wiki/experiments").join(uuid);
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
