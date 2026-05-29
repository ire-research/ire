use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{Emitter, State};

use crate::cc::discovery::find_claude_binary;
use crate::cc::session::SessionManager;
use crate::cc::spawn::{build_command, SpawnArgs};
use crate::cc::stream::{self as cc_stream, StreamEvent, StreamState};
use crate::codex::discovery::find_codex_binary;
use crate::codex::spawn::{build_codex_command, CodexSpawnArgs};
use crate::codex::stream as codex_stream;
use crate::commands::chat::ChatOptions;
use crate::db::models;
use crate::events;
use crate::prompts;
use crate::resources::fetch::fetch_and_extract;
use crate::resources::local::extract_local_file;
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

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceSourceInput {
    Url { url: String },
    LocalFile { path: String },
}

#[derive(Debug, Clone)]
struct PreparedSource {
    source_ref: String,
    source_label: String,
    local_content_sha256: Option<String>,
    cache_rel: String,
    text: String,
    content_type: String,
    source_type: String,
}

#[derive(Debug, Clone)]
struct SummarySource {
    source_ref: String,
    cache_rel: String,
}

#[tauri::command]
pub async fn submit_resource(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    session: State<'_, SessionManager>,
    url: String,
    options: ChatOptions,
) -> Result<String, String> {
    tracing::info!(url = %url, "submit_resource");

    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
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
        vec![SummarySource {
            source_ref: url.clone(),
            cache_rel: format!(".ire/cache/{sha256}.txt"),
        }],
        options,
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
    options: ChatOptions,
) -> Result<String, String> {
    tracing::info!(path = %path, "submit_local_resource");

    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let source_ref = path.clone();
    let path_buf = PathBuf::from(path);
    let workspace_clone = workspace_path.clone();

    let (sha256, source_ref, content_type) =
        tokio::task::spawn_blocking(move || -> Result<(String, String, String), String> {
            let result = extract_local_file(&path_buf).map_err(|e| e.to_string())?;
            let sha256 = sha256_hex_bytes(&result.bytes);

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

            Ok((sha256, source_ref, result.content_type))
        })
        .await
        .map_err(|e| e.to_string())??;

    tracing::debug!(sha256 = %sha256, content_type = %content_type, "local resource cached");

    start_resource_summary(
        app_handle,
        (*session).clone(),
        workspace_path,
        sha256.clone(),
        vec![SummarySource {
            source_ref,
            cache_rel: format!(".ire/cache/{sha256}.txt"),
        }],
        options,
    )?;

    tracing::info!(sha256 = %sha256, "submit_local_resource complete");
    Ok(sha256)
}

#[tauri::command]
pub async fn submit_resources(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    session: State<'_, SessionManager>,
    sources: Vec<ResourceSourceInput>,
    options: ChatOptions,
) -> Result<String, String> {
    tracing::info!(source_count = sources.len(), "submit_resources");

    if sources.is_empty() {
        return Err("at least one resource source is required".to_string());
    }

    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let workspace_clone = workspace_path.clone();
    let (resource_id, summary_sources) =
        tokio::task::spawn_blocking(move || -> Result<(String, Vec<SummarySource>), String> {
            let prepared = prepare_sources(&sources)?;
            let source_refs: Vec<String> = prepared.iter().map(|s| s.source_ref.clone()).collect();
            let resource_id = batch_resource_id(&prepared, &source_refs)?;
            let multi_source = prepared.len() > 1;

            if let Err(e) =
                write_prepared_sources(&workspace_clone, &resource_id, &prepared, multi_source)
            {
                cleanup_resource_cache(&workspace_clone, &resource_id);
                return Err(e);
            }

            let source_label = batch_source_label(&prepared);
            let source_type = if multi_source {
                "batch".to_string()
            } else {
                prepared[0].source_type.clone()
            };
            let content_type = if multi_source {
                "multiple".to_string()
            } else {
                prepared[0].content_type.clone()
            };
            let source_ref = if multi_source {
                serde_json::to_string(&source_refs).map_err(|e| e.to_string())?
            } else {
                prepared[0].source_ref.clone()
            };

            let ire_dir = workspace_clone.join(".ire");
            if let Err(e) = models::insert_resource_with_source(
                &ire_dir,
                &resource_id,
                &source_ref,
                &source_label,
                &source_type,
                &content_type,
            ) {
                cleanup_resource_cache(&workspace_clone, &resource_id);
                return Err(e.to_string());
            }

            let summary_sources = prepared
                .into_iter()
                .map(|source| SummarySource {
                    source_ref: source.source_ref,
                    cache_rel: source.cache_rel.replace("{resource_id}", &resource_id),
                })
                .collect();

            Ok((resource_id, summary_sources))
        })
        .await
        .map_err(|e| e.to_string())??;

    start_resource_summary(
        app_handle,
        (*session).clone(),
        workspace_path,
        resource_id.clone(),
        summary_sources,
        options,
    )?;

    tracing::info!(resource_id = %resource_id, "submit_resources complete");
    Ok(resource_id)
}

fn write_resource_cache(workspace_path: &Path, sha256: &str, text: &str) -> Result<(), String> {
    let cache_dir = workspace_path.join(".ire/cache");
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    fs::write(cache_dir.join(format!("{sha256}.txt")), text).map_err(|e| e.to_string())
}

fn prepare_sources(sources: &[ResourceSourceInput]) -> Result<Vec<PreparedSource>, String> {
    let mut prepared = Vec::with_capacity(sources.len());
    let multi_source = sources.len() > 1;

    for (index, source) in sources.iter().enumerate() {
        let source_number = index + 1;
        let mut item = match source {
            ResourceSourceInput::Url { url } => {
                let trimmed = url.trim();
                if trimmed.is_empty() {
                    return Err(format!("source {source_number}: URL is empty"));
                }
                let result = fetch_and_extract(trimmed)
                    .map_err(|e| format!("source {source_number}: {e}"))?;
                PreparedSource {
                    source_ref: trimmed.to_string(),
                    source_label: hostname_from_url(trimmed),
                    local_content_sha256: None,
                    cache_rel: String::new(),
                    text: result.text,
                    content_type: result.content_type,
                    source_type: "url".to_string(),
                }
            }
            ResourceSourceInput::LocalFile { path } => {
                let path_buf = PathBuf::from(path);
                let result = extract_local_file(&path_buf)
                    .map_err(|e| format!("source {source_number}: {e}"))?;
                let sha256 = sha256_hex_bytes(&result.bytes);
                PreparedSource {
                    source_ref: path.clone(),
                    source_label: result.filename,
                    local_content_sha256: Some(sha256),
                    cache_rel: String::new(),
                    text: result.text,
                    content_type: result.content_type,
                    source_type: "local_file".to_string(),
                }
            }
        };

        item.cache_rel = if multi_source {
            format!(".ire/cache/{{resource_id}}/source-{source_number:03}.txt")
        } else {
            ".ire/cache/{resource_id}.txt".to_string()
        };
        prepared.push(item);
    }

    Ok(prepared)
}

fn batch_resource_id(
    prepared: &[PreparedSource],
    source_refs: &[String],
) -> Result<String, String> {
    if prepared.len() == 1 && prepared[0].source_type == "url" {
        return Ok(sha256_hex(&prepared[0].source_ref));
    }
    if prepared.len() == 1 && prepared[0].source_type == "local_file" {
        if let Some(source_id) = &prepared[0].local_content_sha256 {
            return Ok(source_id.clone());
        }
    }
    let serialized = serde_json::to_string(source_refs).map_err(|e| e.to_string())?;
    Ok(sha256_hex(&serialized))
}

fn write_prepared_sources(
    workspace_path: &Path,
    resource_id: &str,
    prepared: &[PreparedSource],
    multi_source: bool,
) -> Result<(), String> {
    if multi_source {
        let cache_dir = workspace_path.join(".ire/cache").join(resource_id);
        fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
        for (index, source) in prepared.iter().enumerate() {
            fs::write(
                cache_dir.join(format!("source-{:03}.txt", index + 1)),
                &source.text,
            )
            .map_err(|e| e.to_string())?;
        }
    } else if let Some(source) = prepared.first() {
        write_resource_cache(workspace_path, resource_id, &source.text)?;
    }
    Ok(())
}

fn batch_source_label(prepared: &[PreparedSource]) -> String {
    if prepared.len() == 1 {
        return prepared[0].source_label.clone();
    }
    format!("{} sources", prepared.len())
}

fn cleanup_resource_cache(workspace_path: &Path, resource_id: &str) {
    let cache_root = workspace_path.join(".ire/cache");
    let cache_file = cache_root.join(format!("{resource_id}.txt"));
    if cache_file.exists() {
        let _ = fs::remove_file(cache_file);
    }
    let cache_dir = cache_root.join(resource_id);
    if cache_dir.exists() {
        let _ = fs::remove_dir_all(cache_dir);
    }
}

fn start_resource_summary(
    app_handle: tauri::AppHandle,
    session: SessionManager,
    workspace_path: PathBuf,
    sha256: String,
    sources: Vec<SummarySource>,
    options: ChatOptions,
) -> Result<(), String> {
    if options.provider != "claude" && options.provider != "codex" {
        return Err(format!("unsupported provider: {}", options.provider));
    }

    let bin = if options.provider == "codex" {
        find_codex_binary().map_err(|e| e.to_string())?.path
    } else {
        find_claude_binary().map_err(|e| e.to_string())?.path
    };

    let tab_id = uuid::Uuid::new_v4().to_string();
    app_handle
        .emit(
            "tab-created",
            serde_json::json!({
                "tab_id": &tab_id,
                "label": "Ingest",
                "kind": "resource",
                "resource_id": &sha256,
                "agent_options": {
                    "provider": &options.provider,
                    "model": &options.model,
                    "effort": &options.effort,
                },
            }),
        )
        .map_err(|e| e.to_string())?;

    // Fire-and-forget: kick an agent turn to summarise the cached source file(s).
    let app = app_handle.clone();
    let workspace_clone2 = workspace_path.clone();
    let sources_clone = sources.clone();
    let tab_id_clone = tab_id.clone();
    let provider = options.provider.clone();
    let model = options.model.clone();
    let effort = options.effort.clone();
    let sha256_for_log = sha256.clone();

    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let mcp_config = workspace_clone2.join(".ire/mcp.json");
            let system_prompt = build_resource_system_prompt(&workspace_clone2);
            let prompt = build_resource_summary_prompt(&sha256, &sources_clone);

            let mcp_config = if mcp_config.exists() {
                Some(mcp_config)
            } else {
                None
            };

            let mut cmd = if provider == "codex" {
                build_codex_command(&CodexSpawnArgs {
                    bin: &bin,
                    workspace: &workspace_clone2,
                    message: &prompt,
                    model: &model,
                    reasoning_effort: effort.as_deref().unwrap_or("low"),
                    system_prompt: Some(&system_prompt),
                    mcp_config: mcp_config.as_deref(),
                    resume_id: None,
                })
            } else {
                build_command(&SpawnArgs {
                    bin: &bin,
                    workspace: &workspace_clone2,
                    message: &prompt,
                    resume_id: None,
                    mcp_config: mcp_config.as_deref(),
                    system_prompt: Some(&system_prompt),
                    model: &model,
                    effort: effort.as_deref(),
                })
            };

            let mut child = cmd.spawn().map_err(|e| e.to_string())?;
            let pid = child.id();
            let stream_id = format!("{}:{}", tab_id_clone, uuid::Uuid::new_v4());
            let mut event_id = 0_u64;
            session.set_agent_options(&tab_id_clone, &provider, &model, effort.as_deref());
            session.set_pid(&tab_id_clone, pid);
            tracing::info!(tab_id = %tab_id_clone, provider = %provider, pid = pid, "resource agent turn spawned");

            let stdout = child.stdout.take().ok_or("no stdout")?;
            let mut state = StreamState::default();

            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    let mut emit_event = |event: StreamEvent| {
                        if let StreamEvent::Init { ref session_id } = event {
                            session.set_session_id_for_provider(
                                &tab_id_clone,
                                &provider,
                                session_id.clone(),
                            );
                        }
                        event_id += 1;
                        let _ = app.emit(
                            "chat-stream",
                            serde_json::json!({
                                "tab_id": &tab_id_clone,
                                "stream_id": &stream_id,
                                "event_id": event_id,
                                "event": &event,
                            }),
                        );
                    };
                    if provider == "codex" {
                        codex_stream::dispatch(&json, &mut state, &mut emit_event);
                    } else {
                        cc_stream::dispatch(&json, &mut state, &mut emit_event);
                    }
                }
            }

            let _ = child.wait();
            session.clear_pid(&tab_id_clone);

            if !state.emitted_done {
                event_id += 1;
                let _ = app.emit(
                    "chat-stream",
                    serde_json::json!({
                        "tab_id": &tab_id_clone,
                        "stream_id": &stream_id,
                        "event_id": event_id,
                        "event": &StreamEvent::Done,
                    }),
                );
            }

            Ok::<(), String>(())
        })
        .await;

        if let Err(e) = result {
            tracing::warn!(error = %e, "resource agent turn join error");
        }
    });

    tracing::info!(tab_id = %tab_id, sha256 = %sha256_for_log, "resource summary started");
    Ok(())
}

fn build_resource_summary_prompt(resource_id: &str, sources: &[SummarySource]) -> String {
    let draft_path = format!(".ire/cache/{resource_id}_draft.md");
    let today = chrono::Utc::now().format("%Y-%m-%d");

    if sources.len() == 1 {
        let source = &sources[0];
        return format!(
            "Read {} (source: {}).\nDraft path: {draft_path}\nToday: {today}\n\
             Follow the resource analyst instructions in your system prompt.",
            source.cache_rel, source.source_ref
        );
    }

    let mut prompt = format!(
        "Read all cached source files in order.\nDraft path: {draft_path}\nToday: {today}\n\nSources:\n"
    );
    for (index, source) in sources.iter().enumerate() {
        prompt.push_str(&format!(
            "{}. {} (source: {})\n",
            index + 1,
            source.cache_rel,
            source.source_ref
        ));
    }
    prompt.push_str("\nFollow the resource analyst instructions in your system prompt.");
    prompt
}

#[tauri::command]
pub fn read_resource_draft(
    active: State<'_, ActiveWorkspace>,
    resource_id: String,
) -> Result<String, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };
    let draft_path = workspace_path
        .join(".ire/cache")
        .join(format!("{resource_id}_draft.md"));
    let content = fs::read_to_string(&draft_path).map_err(|e| e.to_string())?;
    normalize_resource_draft_sources(&workspace_path, &resource_id, &content)
}

#[tauri::command]
pub fn confirm_resource(
    app: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    resource_id: String,
) -> Result<(), String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let draft_path = workspace_path
        .join(".ire/cache")
        .join(format!("{resource_id}_draft.md"));
    let content = fs::read_to_string(&draft_path).map_err(|e| e.to_string())?;
    let content = normalize_resource_draft_sources(&workspace_path, &resource_id, &content)?;

    let (fm, _) = crate::wiki::frontmatter::parse(&content);
    let title = fm
        .as_ref()
        .and_then(|m| m.get("title"))
        .map(|t| sanitize_wiki_filename(t))
        .unwrap_or_else(|| resource_id[..8.min(resource_id.len())].to_string());

    let wiki_path = format!("resources/{title}.md");
    let store = crate::wiki::WikiStore::new(workspace_path.clone());
    store
        .write(&wiki_path, &content, &app)
        .map_err(|e| e.to_string())?;

    cleanup_resource_cache(&workspace_path, &resource_id);
    let _ = fs::remove_file(&draft_path);

    Ok(())
}

fn sanitize_wiki_filename(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn normalize_resource_draft_sources(
    workspace_path: &Path,
    resource_id: &str,
    content: &str,
) -> Result<String, String> {
    let ire_dir = workspace_path.join(".ire");
    let Some(row) = models::get_resource(&ire_dir, resource_id).map_err(|e| e.to_string())? else {
        return Ok(content.to_string());
    };
    let source_refs = stored_source_refs(&row.url);
    if source_refs.is_empty() {
        return Ok(content.to_string());
    }
    Ok(replace_frontmatter_sources(content, &source_refs))
}

fn stored_source_refs(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_else(|_| vec![value.to_string()])
}

fn replace_frontmatter_sources(content: &str, source_refs: &[String]) -> String {
    if !content.starts_with("---\n") || source_refs.is_empty() {
        return content.to_string();
    }

    let rest = &content[4..];
    let Some(end) = rest.find("\n---") else {
        return content.to_string();
    };

    let fm_text = &rest[..end];
    let suffix = &rest[end..];
    let lines: Vec<&str> = fm_text.lines().collect();
    let mut next_lines = Vec::new();
    let mut inserted = false;
    let mut i = 0;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    while i < lines.len() {
        let line = lines[i];

        if frontmatter_key(line) == Some("updated") {
            i += 1;
            continue;
        }

        if frontmatter_key(line) == Some("sources") {
            if !inserted {
                push_sources_block(&mut next_lines, source_refs);
                next_lines.push(format!("updated: {today}"));
                inserted = true;
            }
            i += 1;
            while i < lines.len() && is_sources_continuation(lines[i]) {
                i += 1;
            }
            continue;
        }

        next_lines.push(line.to_string());
        if !inserted && frontmatter_key(line) == Some("title") {
            push_sources_block(&mut next_lines, source_refs);
            next_lines.push(format!("updated: {today}"));
            inserted = true;
        }
        i += 1;
    }

    if !inserted {
        push_sources_block(&mut next_lines, source_refs);
        next_lines.push(format!("updated: {today}"));
    }

    format!("---\n{}{}", next_lines.join("\n"), suffix)
}

fn frontmatter_key(line: &str) -> Option<&str> {
    line.split_once(':').map(|(key, _)| key.trim())
}

fn is_sources_continuation(line: &str) -> bool {
    line.trim_start().starts_with("- ") || line.starts_with(' ') || line.starts_with('\t')
}

fn push_sources_block(lines: &mut Vec<String>, source_refs: &[String]) {
    lines.push("sources:".to_string());
    for source_ref in source_refs {
        lines.push(format!("  - {source_ref}"));
    }
}

#[tauri::command]
pub fn discard_resource(
    app: tauri::AppHandle,
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

    cleanup_resource_cache(&workspace_path, &resource_id);

    let ire_dir = workspace_path.join(".ire");

    if let Ok(Some(row)) = models::get_resource(&ire_dir, &resource_id) {
        if let Some(wiki_path) = row.wiki_path {
            let store = crate::wiki::WikiStore::new(workspace_path.clone());
            if let Err(e) = store.delete(&wiki_path) {
                tracing::warn!(wiki_path = %wiki_path, error = %e, "discard_resource: failed to delete wiki file");
            }
        }
    }

    models::update_resource_status(&ire_dir, &resource_id, "rejected")
        .map_err(|e| e.to_string())?;

    events::emit_resource_deleted(&app, &resource_id);
    Ok(())
}

fn build_resource_system_prompt(workspace_root: &Path) -> String {
    let ire_root = workspace_root.join(".ire");
    let wiki_root = workspace_root.join(".ire/wiki");
    let mut parts: Vec<String> = Vec::new();

    if let Ok(content) = fs::read_to_string(ire_root.join("_SYSTEM.md")) {
        if !content.trim().is_empty() {
            parts.push(content);
        }
    }

    parts.push(prompts::resource_summarizer().to_string());

    for rel in &["pulse.json", "_index.md"] {
        if let Ok(content) = fs::read_to_string(wiki_root.join(rel)) {
            if !content.trim().is_empty() {
                parts.push(format!("### {rel}\n\n{content}"));
            }
        }
    }

    parts.join("\n\n---\n\n")
}

#[cfg(test)]
mod tests {
    use super::{
        batch_resource_id, build_resource_summary_prompt, prepare_sources,
        replace_frontmatter_sources, ResourceSourceInput,
    };

    #[test]
    fn local_file_source_ref_uses_exact_input_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("paper.txt");
        std::fs::write(&file, "paper text").unwrap();
        let path = file.to_string_lossy().to_string();

        let prepared =
            prepare_sources(&[ResourceSourceInput::LocalFile { path: path.clone() }]).unwrap();

        assert_eq!(prepared[0].source_ref, path);
        assert!(!prepared[0].source_ref.starts_with("file:"));
        assert_eq!(
            batch_resource_id(&prepared, &[prepared[0].source_ref.clone()]).unwrap(),
            prepared[0]
                .local_content_sha256
                .as_ref()
                .unwrap()
                .to_string()
        );
    }

    #[test]
    fn summary_prompt_uses_exact_local_path_as_source() {
        let path = "/Users/me/Documents/paper.pdf";
        let prompt = build_resource_summary_prompt(
            "abc123",
            &[super::SummarySource {
                source_ref: path.to_string(),
                cache_rel: ".ire/cache/abc123.txt".to_string(),
            }],
        );

        assert!(prompt.contains("(source: /Users/me/Documents/paper.pdf)"));
        assert!(!prompt.contains("file:abc123"));
    }

    #[test]
    fn replace_frontmatter_sources_overwrites_uuid_source() {
        let content = "---\ntitle: \"Paper\"\nsources:\n  - .ire/cache/012345abcdef.txt\nupdated: 2026-05-29\nTL;DR: \"Relevant\"\n---\n\n# Paper\n";
        let source_refs = vec!["/Users/me/Documents/paper.pdf".to_string()];

        let normalized = replace_frontmatter_sources(content, &source_refs);

        assert!(normalized.contains("sources:\n  - /Users/me/Documents/paper.pdf\nupdated:"));
        assert!(!normalized.contains(".ire/cache/012345abcdef.txt"));
    }

    #[test]
    fn replace_frontmatter_sources_preserves_two_source_order() {
        let content = "---\ntitle: \"Pair\"\nsources:\n  - wrong-a\n  - wrong-b\nupdated: 2026-05-28\nTL;DR: \"Relevant\"\n---\n\n# Pair\n";
        let source_refs = vec![
            "/Users/me/Documents/a.pdf".to_string(),
            "https://example.com/b".to_string(),
        ];

        let normalized = replace_frontmatter_sources(content, &source_refs);

        assert!(normalized.contains(
            "sources:\n  - /Users/me/Documents/a.pdf\n  - https://example.com/b\nupdated:"
        ));
        assert!(!normalized.contains("wrong-a"));
        assert!(!normalized.contains("wrong-b"));
    }

    #[test]
    fn replace_frontmatter_sources_preserves_multiple_source_order() {
        let content = "---\ntitle: \"Batch\"\nsources: [.ire/cache/batch/source-001.txt]\nupdated: 2026-05-28\nTL;DR: \"Relevant\"\n---\n\n# Batch\n";
        let source_refs = vec![
            "https://example.com/a".to_string(),
            "/Users/me/Documents/b.pdf".to_string(),
            "https://example.com/c".to_string(),
        ];

        let normalized = replace_frontmatter_sources(content, &source_refs);

        assert!(normalized.contains(
            "sources:\n  - https://example.com/a\n  - /Users/me/Documents/b.pdf\n  - https://example.com/c\nupdated:"
        ));
        assert!(!normalized.contains(".ire/cache/batch/source-001.txt"));
    }

    #[test]
    fn replace_frontmatter_sources_inserts_after_title_when_missing() {
        let content =
            "---\ntitle: \"Paper\"\nupdated: 2026-05-29\nTL;DR: \"Relevant\"\n---\n\n# Paper\n";
        let source_refs = vec!["/Users/me/Documents/paper.pdf".to_string()];

        let normalized = replace_frontmatter_sources(content, &source_refs);

        assert!(normalized
            .contains("title: \"Paper\"\nsources:\n  - /Users/me/Documents/paper.pdf\nupdated:"));
    }
}
