use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Local;
use tauri::{AppHandle, Emitter};

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
    app: AppHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    let workspace_root = Arc::new(workspace_root);
    tauri::async_runtime::spawn(async move {
        run_server(socket_path, workspace_root, app).await;
    })
}

#[cfg(not(unix))]
pub fn start(
    _socket_path: PathBuf,
    _workspace_root: PathBuf,
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
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, workspace_root, app).await {
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
        dispatch(&method, &params, &workspace_root, &app)
    })
    .await
    .map_err(|e| anyhow!("task join error: {e}"))??;

    let resp = serde_json::json!({ "id": id, "ok": true, "result": result });
    let mut bytes = serde_json::to_vec(&resp)?;
    bytes.push(b'\n');
    write_half.write_all(&bytes).await?;
    Ok(())
}

fn dispatch(
    method: &str,
    params: &serde_json::Value,
    workspace_root: &Path,
    app: &AppHandle,
) -> Result<serde_json::Value> {
    let wiki = WikiStore::new(workspace_root.to_path_buf());
    match method {
        "wiki.read" => wiki_read(params, &wiki),
        "wiki.write" => wiki_write(params, &wiki, app),
        "wiki.append" => wiki_append(params, &wiki, app),
        "wiki.list" => wiki_list(&wiki),
        "wiki.rename" => wiki_rename(params, &wiki, app),
        "memory.write_long_term" => memory_write_long_term(params, &wiki, app),
        "memory.write_short_term" => memory_write_short_term(params, &wiki, app),
        "memory.record_failure" => memory_record_failure(params, &wiki, app),
        "pulse.update" => pulse_update(params, &wiki, app),
        _ => Err(anyhow!("unknown method: {method}")),
    }
}

fn wiki_read(params: &serde_json::Value, wiki: &WikiStore) -> Result<serde_json::Value> {
    let path = params["path"].as_str().ok_or(anyhow!("missing path"))?;
    let (content, frontmatter) = wiki.read(path)?;
    Ok(serde_json::json!({ "content": content, "frontmatter": frontmatter }))
}

fn wiki_write(params: &serde_json::Value, wiki: &WikiStore, app: &AppHandle) -> Result<serde_json::Value> {
    let path = params["path"].as_str().ok_or(anyhow!("missing path"))?;
    let content = params["content"].as_str().ok_or(anyhow!("missing content"))?;
    wiki.write(path, content, app)?;
    Ok(serde_json::json!({ "written": path }))
}

fn wiki_append(params: &serde_json::Value, wiki: &WikiStore, app: &AppHandle) -> Result<serde_json::Value> {
    let path = params["path"].as_str().ok_or(anyhow!("missing path"))?;
    let content = params["content"].as_str().ok_or(anyhow!("missing content"))?;
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
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let rel = path.strip_prefix(wiki_root)?.to_string_lossy().replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

fn wiki_rename(params: &serde_json::Value, wiki: &WikiStore, app: &AppHandle) -> Result<serde_json::Value> {
    let from = params["from"].as_str().ok_or(anyhow!("missing from"))?;
    let to = params["to"].as_str().ok_or(anyhow!("missing to"))?;
    wiki.rename(from, to, app)?;
    Ok(serde_json::json!({ "renamed": { "from": from, "to": to } }))
}

fn memory_write_long_term(params: &serde_json::Value, wiki: &WikiStore, app: &AppHandle) -> Result<serde_json::Value> {
    let section = params["section"].as_str().ok_or(anyhow!("missing section"))?;
    let content = params["content"].as_str().ok_or(anyhow!("missing content"))?;

    let path = "status/long-term.md";
    let existing = fs::read_to_string(wiki.wiki_root.join(path)).unwrap_or_default();
    let merged = format!("{existing}\n## {section}\n\n{content}\n");
    wiki.write(path, &merged, app)?;
    Ok(serde_json::json!({ "written": path }))
}

fn memory_write_short_term(params: &serde_json::Value, wiki: &WikiStore, app: &AppHandle) -> Result<serde_json::Value> {
    let content = params["content"].as_str().ok_or(anyhow!("missing content"))?;

    let today = Local::now().format("%Y-%m-%d").to_string();
    let path = format!("status/short-term/{today}.md");
    let existing = fs::read_to_string(wiki.wiki_root.join(&path)).unwrap_or_default();
    let merged = format!("{existing}\n{content}\n");
    wiki.write(&path, &merged, app)?;
    Ok(serde_json::json!({ "written": path }))
}

fn memory_record_failure(params: &serde_json::Value, wiki: &WikiStore, app: &AppHandle) -> Result<serde_json::Value> {
    let method = params["method"].as_str().ok_or(anyhow!("missing method"))?;
    let reason = params["reason"].as_str().ok_or(anyhow!("missing reason"))?;
    let context_ref = params["context_ref"].as_str().unwrap_or("");

    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut entry = format!("\n## {method}\n- **why-it-failed**: {reason}\n- **recorded**: {today}\n");
    if !context_ref.is_empty() {
        entry.push_str(&format!("- **context**: {context_ref}\n"));
    }

    let path = "status/failures.md";
    let existing = fs::read_to_string(wiki.wiki_root.join(path)).unwrap_or_default();
    wiki.write(path, &(existing + &entry), app)?;
    Ok(serde_json::json!({ "recorded": method }))
}

fn pulse_update(params: &serde_json::Value, wiki: &WikiStore, app: &AppHandle) -> Result<serde_json::Value> {
    let question = params["question"].as_str();
    let blocker = params["blocker"].as_str();
    let focus = params["focus"].as_str();

    let path = "status/pulse.md";
    let existing = fs::read_to_string(wiki.wiki_root.join(path)).unwrap_or_default();
    let updated = patch_pulse(&existing, question, blocker, focus);
    wiki.write(path, &updated, app)?;

    let (q, b, f) = parse_pulse_fields(&updated);
    let _ = app.emit("pulse-changed", serde_json::json!({ "question": q, "blocker": b, "focus": f }));

    Ok(serde_json::json!({ "updated": path }))
}

fn patch_pulse(
    content: &str,
    question: Option<&str>,
    blocker: Option<&str>,
    focus: Option<&str>,
) -> String {
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut saw_q = false;
    let mut saw_b = false;
    let mut saw_f = false;

    for line in &mut lines {
        if line.starts_with("**Question:**") {
            saw_q = true;
            if let Some(q) = question {
                *line = format!("**Question:** {q}");
            }
        } else if line.starts_with("**Blocker:**") {
            saw_b = true;
            if let Some(b) = blocker {
                *line = format!("**Blocker:** {b}");
            }
        } else if line.starts_with("**Focus:**") {
            saw_f = true;
            if let Some(f) = focus {
                *line = format!("**Focus:** {f}");
            }
        }
    }

    if !saw_q {
        if let Some(q) = question {
            lines.push(format!("**Question:** {q}"));
        }
    }
    if !saw_b {
        if let Some(b) = blocker {
            lines.push(format!("**Blocker:** {b}"));
        }
    }
    if !saw_f {
        if let Some(f) = focus {
            lines.push(format!("**Focus:** {f}"));
        }
    }

    lines.join("\n")
}

fn parse_pulse_fields(content: &str) -> (Option<String>, Option<String>, Option<String>) {
    let (mut q, mut b, mut f) = (None, None, None);
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("**Question:**") {
            q = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("**Blocker:**") {
            b = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("**Focus:**") {
            f = Some(rest.trim().to_string());
        }
    }
    (q, b, f)
}
