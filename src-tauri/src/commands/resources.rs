use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{Emitter, State};

use crate::cc::discovery::find_claude_binary;
use crate::cc::session::SessionManager;
use crate::cc::spawn::{build_command, SpawnArgs};
use crate::cc::stream::{dispatch, StreamEvent, StreamState};
use crate::db::models;
use crate::prompts;
use crate::resources::fetch::fetch_and_extract;
use crate::resources::local::extract_local_file;
use crate::wiki::WikiStore;
use crate::workspace::state::ActiveWorkspace;

fn sha256_hex(s: &str) -> String {
    sha256_hex_bytes(s.as_bytes())
}

fn sha256_hex_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(bytes);
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

fn hostname_from_url(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("resource")
        .to_string()
}

#[derive(Debug, Serialize, Clone)]
pub struct ResourceItem {
    pub resource_id: String,
    pub url: String,
    pub source_type: String,
    pub source_label: String,
    pub title: Option<String>,
    pub wiki_path: Option<String>,
}

#[tauri::command]
pub async fn submit_resource(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    session: State<'_, SessionManager>,
    url: String,
) -> Result<String, String> {
    tracing::info!(url = %url, "submit_resource");

    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };

    let sha256 = sha256_hex(&url);
    let url_clone = url.clone();
    let sha256_clone = sha256.clone();
    let workspace_clone = workspace_path.clone();

    let content_type = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let result = fetch_and_extract(&url_clone).map_err(|e| e.to_string())?;

        write_resource_cache(&workspace_clone, &sha256_clone, &result.text)?;

        let ire_dir = workspace_clone.join(".ire");
        models::insert_resource(&ire_dir, &sha256_clone, &url_clone, &result.content_type)
            .map_err(|e| e.to_string())?;

        Ok(result.content_type)
    })
    .await
    .map_err(|e| e.to_string())??;

    tracing::debug!(sha256 = %sha256, content_type = %content_type, "resource cached");

    start_resource_summary(
        app_handle,
        (*session).clone(),
        workspace_path,
        sha256.clone(),
        url.clone(),
        hostname_from_url(&url),
    )?;

    tracing::info!(sha256 = %sha256, "submit_resource complete");
    Ok(sha256)
}

#[tauri::command]
pub async fn submit_local_resource(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    session: State<'_, SessionManager>,
    path: String,
) -> Result<String, String> {
    tracing::info!(path = %path, "submit_local_resource");

    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };

    let path_buf = PathBuf::from(path);
    let workspace_clone = workspace_path.clone();

    let (sha256, source_ref, source_label, content_type) = tokio::task::spawn_blocking(
        move || -> Result<(String, String, String, String), String> {
            let result = extract_local_file(&path_buf).map_err(|e| e.to_string())?;
            let sha256 = sha256_hex_bytes(&result.bytes);
            let source_ref = format!("file:{sha256}:{}", result.filename);

            write_resource_cache(&workspace_clone, &sha256, &result.text)?;

            let ire_dir = workspace_clone.join(".ire");
            models::insert_resource_with_source(
                &ire_dir,
                &sha256,
                &source_ref,
                &result.filename,
                "local_file",
                &result.content_type,
            )
            .map_err(|e| e.to_string())?;

            Ok((sha256, source_ref, result.filename, result.content_type))
        },
    )
    .await
    .map_err(|e| e.to_string())??;

    tracing::debug!(sha256 = %sha256, content_type = %content_type, "local resource cached");

    start_resource_summary(
        app_handle,
        (*session).clone(),
        workspace_path,
        sha256.clone(),
        source_ref,
        source_label,
    )?;

    tracing::info!(sha256 = %sha256, "submit_local_resource complete");
    Ok(sha256)
}

fn write_resource_cache(workspace_path: &Path, sha256: &str, text: &str) -> Result<(), String> {
    let cache_dir = workspace_path.join(".ire/cache");
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    fs::write(cache_dir.join(format!("{sha256}.txt")), text).map_err(|e| e.to_string())
}

fn start_resource_summary(
    app_handle: tauri::AppHandle,
    session: SessionManager,
    workspace_path: PathBuf,
    sha256: String,
    source_ref: String,
    tab_label: String,
) -> Result<(), String> {
    let tab_id = uuid::Uuid::new_v4().to_string();
    app_handle
        .emit(
            "tab-created",
            serde_json::json!({
                "tab_id": &tab_id,
                "label": &tab_label,
                "kind": "resource",
                "resource_id": &sha256,
            }),
        )
        .map_err(|e| e.to_string())?;

    // Fire-and-forget: kick a CC turn to summarise the cached file
    let bin = find_claude_binary().map_err(|e| e.to_string())?.path;
    let app = app_handle.clone();
    let workspace_clone2 = workspace_path.clone();
    let sha256_clone2 = sha256.clone();
    let source_ref_clone = source_ref.clone();
    let tab_id_clone = tab_id.clone();

    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let mcp_config = workspace_clone2.join(".ire/mcp.json");
            let system_prompt = build_resource_system_prompt(&workspace_clone2);
            let cache_rel = format!(".ire/cache/{sha256_clone2}.txt");
            let prompt = format!(
                "Read {cache_rel} (source: {source_ref_clone}). \
                 Provide an executive summary — what this resource is, what is relevant to this project, \
                 why it matters, and how it could be used. Use bullet points.\n\
                 Do NOT write to the wiki yet."
            );

            let mut cmd = build_command(&SpawnArgs {
                bin: &bin,
                workspace: &workspace_clone2,
                message: &prompt,
                resume_id: None,
                mcp_config: if mcp_config.exists() { Some(&mcp_config) } else { None },
                system_prompt: Some(&system_prompt),
                model: "claude-haiku-4-5-20251001",
                effort: "high",
            });

            let mut child = cmd.spawn().map_err(|e| e.to_string())?;
            let pid = child.id();
            session.set_pid(&tab_id_clone, pid);
            tracing::info!(tab_id = %tab_id_clone, pid = pid, "resource CC turn spawned");

            let stdout = child.stdout.take().ok_or("no stdout")?;
            let mut state = StreamState::default();

            for line in BufReader::new(stdout).lines() {
                if let Ok(line) = line {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                        dispatch(&json, &mut state, &mut |event| {
                            if let StreamEvent::Init { ref session_id } = event {
                                session.set_session_id(&tab_id_clone, session_id.clone());
                            }
                            let _ = app.emit(
                                "chat-stream",
                                serde_json::json!({ "tab_id": &tab_id_clone, "event": &event }),
                            );
                        });
                    }
                }
            }

            let _ = child.wait();
            session.clear_pid(&tab_id_clone);

            let _ = app.emit(
                "chat-stream",
                serde_json::json!({ "tab_id": &tab_id_clone, "event": &StreamEvent::Done }),
            );

            Ok::<(), String>(())
        })
        .await;

        if let Err(e) = result {
            tracing::warn!(error = %e, "resource CC turn join error");
        }
    });

    tracing::info!(tab_id = %tab_id, sha256 = %sha256, "resource summary started");
    Ok(())
}

#[tauri::command]
pub fn discard_resource(
    active: State<'_, ActiveWorkspace>,
    resource_id: String,
) -> Result<(), String> {
    tracing::info!(resource_id = %resource_id, "discard_resource");
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let cache_file = workspace_path
        .join(".ire/cache")
        .join(format!("{resource_id}.txt"));
    if cache_file.exists() {
        let _ = fs::remove_file(&cache_file);
    }

    let ire_dir = workspace_path.join(".ire");
    models::update_resource_status(&ire_dir, &resource_id, "rejected")
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn index_resource(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    resource_id: String,
) -> Result<(), String> {
    tracing::info!(resource_id = %resource_id, "index_resource");
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let ire_dir = workspace_path.join(".ire");
    let wiki = WikiStore::new(workspace_path.clone());

    // Find the wiki file CC wrote for this resource and extract its title.
    let resource_url = models::get_resource_url(&ire_dir, &resource_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("resource {resource_id} not found in DB"))?;

    let wiki_rel = find_resource_wiki_path(&wiki.wiki_root, &resource_url);

    if let Some(ref rel_path) = wiki_rel {
        let title = extract_title(&wiki.wiki_root.join(rel_path));
        models::update_resource_indexed(&ire_dir, &resource_id, rel_path, &title)
            .map_err(|e| e.to_string())?;
        // Trigger the resources list refresh in the frontend.
        let _ = app_handle.emit("wiki-changed", serde_json::json!({ "path": rel_path }));
    } else {
        // CC didn't write the file (shouldn't happen in normal flow).
        tracing::warn!(resource_id = %resource_id, "no wiki file found for resource — marking summarized without path");
        models::update_resource_status(&ire_dir, &resource_id, "summarized")
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Scan `wiki/resources/` for a `.md` file whose frontmatter `sources:` array
/// contains `resource_url`. `_schema.md` makes `sources` the canonical field.
fn find_resource_wiki_path(wiki_root: &std::path::Path, resource_url: &str) -> Option<String> {
    use crate::wiki::frontmatter;

    let resources_dir = wiki_root.join("resources");
    if !resources_dir.exists() {
        return None;
    }

    for entry in std::fs::read_dir(&resources_dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let (fm, _) = frontmatter::parse(&content);
        let Some(fm) = fm else { continue };
        let Some(sources) = fm.get("sources") else {
            continue;
        };
        if parse_sources_array(sources)
            .iter()
            .any(|s| *s == resource_url)
        {
            let filename = path.file_name()?.to_str()?;
            return Some(format!("resources/{filename}"));
        }
    }
    None
}

/// Parse a YAML inline-array string like `[https://a, "https://b"]` into entries.
fn parse_sources_array(value: &str) -> Vec<&str> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches(|c: char| c == '"' || c == '\''))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract a human title from a wiki markdown file.
/// Priority: frontmatter `title:` → first `# ` heading → filename stem.
fn extract_title(path: &std::path::Path) -> String {
    use crate::wiki::frontmatter;

    let content = std::fs::read_to_string(path).unwrap_or_default();
    let (fm, body) = frontmatter::parse(&content);

    if let Some(fm) = fm {
        if let Some(t) = fm.get("title") {
            let t = t.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }

    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            let h = heading.trim();
            if !h.is_empty() {
                return h.to_string();
            }
        }
    }

    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("resource")
        .to_string()
}

#[tauri::command]
pub fn list_resources(active: State<'_, ActiveWorkspace>) -> Result<Vec<ResourceItem>, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let ire_dir = workspace_path.join(".ire");
    let rows = models::list_resources(&ire_dir).map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let source_label = r.source_label.unwrap_or_else(|| r.url.clone());
            ResourceItem {
                resource_id: r.url_sha256,
                url: r.url,
                source_type: r.source_type,
                source_label,
                title: r.title,
                wiki_path: r.wiki_path,
            }
        })
        .collect())
}

fn build_resource_system_prompt(workspace_root: &Path) -> String {
    let wiki_root = workspace_root.join(".ire/wiki");
    let mut parts: Vec<String> = Vec::new();

    if let Ok(content) = fs::read_to_string(wiki_root.join("_SYSTEM.md")) {
        if !content.trim().is_empty() {
            parts.push(content);
        }
    }

    parts.push(prompts::resource_summarizer().to_string());

    for rel in &["status/pulse.md", "_index.md"] {
        if let Ok(content) = fs::read_to_string(wiki_root.join(rel)) {
            if !content.trim().is_empty() {
                parts.push(format!("### {rel}\n\n{content}"));
            }
        }
    }

    parts.join("\n\n---\n\n")
}

#[tauri::command]
pub fn get_resource_confirm_prompt() -> &'static str {
    prompts::resource_confirm()
}
