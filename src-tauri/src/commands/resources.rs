use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::agent_provider::{self, TurnRequest, TurnTransport};
use crate::commands::chat::ChatOptions;
use crate::opencode::runtime::OpenCodeRuntime;
use crate::prompts;
use crate::resources::fetch::fetch_and_extract;
use crate::resources::local::extract_local_file;
use crate::session::SessionManager;
use crate::stream_event::{StreamEvent, StreamState};
use crate::ire::{focus_prompt_block, IreStore};
use crate::workspace::state::ActiveWorkspace;

/// In-flight ingestion state, keyed by the transient resource id. Holds the
/// source refs (URLs / file paths) injected into the resource's frontmatter on
/// confirm. Entries live only between ingest start and confirm/discard;
/// confirmed resources are identified by their file path instead.
#[derive(Default)]
pub struct InflightResources(pub Mutex<HashMap<String, Vec<String>>>);

impl InflightResources {
    fn register(&self, id: &str, sources: Vec<String>) {
        self.0.lock().unwrap().insert(id.to_string(), sources);
    }

    fn sources(&self, id: &str) -> Vec<String> {
        self.0.lock().unwrap().get(id).cloned().unwrap_or_default()
    }

    fn remove(&self, id: &str) -> Option<Vec<String>> {
        self.0.lock().unwrap().remove(id)
    }
}

fn sha256_hex(s: &str) -> String {
    sha256_hex_bytes(s.as_bytes())
}

/// Truncated to 8 bytes (16 hex chars) — this id only needs to be unique
/// within one workspace's transient `.ire/cache/`, not cryptographically
/// collision-resistant, and it gets echoed into resource-summary prompts as
/// a file path, so the full 32-byte digest just burns context tokens.
fn sha256_hex_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(bytes);
    hash.iter().take(8).map(|b| format!("{b:02x}")).collect()
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
    local_content_sha256: Option<String>,
    cache_rel: String,
    text: String,
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
    inflight: State<'_, InflightResources>,
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
        Ok(result.content_type)
    })
    .await
    .map_err(|e| e.to_string())??;

    inflight.register(&sha256, vec![url.clone()]);
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
    inflight: State<'_, InflightResources>,
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
            Ok((sha256, source_ref, result.content_type))
        })
        .await
        .map_err(|e| e.to_string())??;

    inflight.register(&sha256, vec![source_ref.clone()]);
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
    inflight: State<'_, InflightResources>,
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
    let (resource_id, summary_sources, source_refs) = tokio::task::spawn_blocking(
        move || -> Result<(String, Vec<SummarySource>, Vec<String>), String> {
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

            let summary_sources = prepared
                .into_iter()
                .map(|source| SummarySource {
                    source_ref: source.source_ref,
                    cache_rel: source.cache_rel.replace("{resource_id}", &resource_id),
                })
                .collect();

            Ok((resource_id, summary_sources, source_refs))
        },
    )
    .await
    .map_err(|e| e.to_string())??;

    inflight.register(&resource_id, source_refs);

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
                    local_content_sha256: None,
                    cache_rel: String::new(),
                    text: result.text,
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
                    local_content_sha256: Some(sha256),
                    cache_rel: String::new(),
                    text: result.text,
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

/// Emits the terminal events a failed OpenCode resource-summary send never
/// gets otherwise, so the resource tab (already showing "summarizing") can
/// leave that state instead of hanging forever, and clears the running turn
/// (if `turn::send` registered one before failing).
fn emit_resource_summary_failure(app: &AppHandle, session: &SessionManager, tab_id: &str, message: String) {
    if let Some((running, _)) = session.get_running_and_provider(tab_id) {
        session.clear_running_if(tab_id, &running);
    }
    let stream_id = format!("{tab_id}:{}", uuid::Uuid::new_v4());
    crate::opencode::runtime::emit_stream(app, tab_id, &stream_id, 1, &StreamEvent::Error { message });
    crate::opencode::runtime::emit_stream(app, tab_id, &stream_id, 2, &StreamEvent::Done);
}

fn start_resource_summary(
    app_handle: tauri::AppHandle,
    session: SessionManager,
    workspace_path: PathBuf,
    sha256: String,
    sources: Vec<SummarySource>,
    options: ChatOptions,
) -> Result<(), String> {
    let agent = agent_provider::provider(&options.provider)
        .ok_or_else(|| format!("unsupported provider: {}", options.provider))?;

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

    if agent.transport() == TurnTransport::OpenCodeServer {
        tokio::spawn(async move {
            let Ok(home_data_dir) = crate::workspace::init::require_home_data_dir(&workspace_clone2) else {
                tracing::error!(tab_id = %tab_id_clone, "resource summary: cannot resolve home data dir");
                emit_resource_summary_failure(
                    &app,
                    &session,
                    &tab_id_clone,
                    "resource summary: cannot resolve home data dir".to_string(),
                );
                return;
            };
            let system_prompt = build_resource_system_prompt(&workspace_clone2);
            let prompt = build_resource_summary_prompt(&sha256, &sources_clone);
            let started_at = chrono::Local::now().to_rfc3339();
            let runtime = app.state::<OpenCodeRuntime>();
            let result = crate::opencode::turn::send(
                &app,
                &runtime,
                &session,
                &workspace_clone2,
                &home_data_dir,
                crate::opencode::turn::SendArgs {
                    tab_id: &tab_id_clone,
                    session_uuid: &tab_id_clone,
                    tab_label: "Ingest",
                    started_at: &started_at,
                    model: &model,
                    effort: effort.as_deref(),
                    message: &prompt,
                    system_prompt: Some(&system_prompt),
                },
            )
            .await;
            if let Err(e) = result {
                tracing::warn!(tab_id = %tab_id_clone, error = %e, "resource agent turn (opencode) failed");
                emit_resource_summary_failure(
                    &app,
                    &session,
                    &tab_id_clone,
                    format!("resource summary failed: {e}"),
                );
            }
        });
        tracing::info!(tab_id = %tab_id, sha256 = %sha256_for_log, "resource summary started");
        return Ok(());
    }

    let cli = agent_provider::cli_turn(&options.provider)
        .ok_or_else(|| format!("provider {} has no CLI turn support", options.provider))?;
    let bin = agent.discover().map_err(|e| e.to_string())?.path;

    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let home_data_dir = crate::workspace::init::require_home_data_dir(&workspace_clone2)?;
            let mcp_config = home_data_dir.join("mcp.json");
            let system_prompt = build_resource_system_prompt(&workspace_clone2);
            let prompt = build_resource_summary_prompt(&sha256, &sources_clone);

            let mcp_config = if mcp_config.exists() {
                Some(mcp_config)
            } else {
                None
            };

            let mut cmd = cli.build_command(
                &bin,
                &TurnRequest {
                    workspace: &workspace_clone2,
                    message: &prompt,
                    model: &model,
                    effort: effort.as_deref(),
                    resume_id: None,
                    mcp_config: mcp_config.as_deref(),
                    system_prompt: Some(&system_prompt),
                },
            );

            let mut child = cmd
                .spawn()
                .map_err(|e| cli.normalize_spawn_error(&e).to_string())?;
            let pid = child.id();
            let stream_id = format!("{}:{}", tab_id_clone, uuid::Uuid::new_v4());
            let mut event_id = 0_u64;
            let started_at_res = chrono::Local::now().to_rfc3339();
            session.set_agent_options(&tab_id_clone, &tab_id_clone, &provider, &model, effort.as_deref());
            session.set_pid(&tab_id_clone, pid);
            tracing::info!(tab_id = %tab_id_clone, provider = %provider, pid = pid, "resource agent turn spawned");

            let stdout = child.stdout.take().ok_or("no stdout")?;
            let mut state = StreamState::default();

            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    let mut emit_event = |event: StreamEvent| {
                        if let StreamEvent::Init { ref session_id } = event {
                            let _ = crate::db::models::upsert_chat_resume_id(
                                &home_data_dir,
                                &tab_id_clone,
                                "Ingest",
                                &provider,
                                &model,
                                &started_at_res,
                                session_id,
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
                    cli.dispatch(&json, &mut state, &mut emit_event);
                }
            }

            let _ = child.wait();
            session.clear_running_if(&tab_id_clone, &crate::session::RunningTurn::Process(pid));

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
    inflight: State<'_, InflightResources>,
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
    Ok(normalize_resource_draft_sources(
        &content,
        &inflight.sources(&resource_id),
    ))
}

#[tauri::command]
pub fn confirm_resource(
    app: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    inflight: State<'_, InflightResources>,
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
    let content = normalize_resource_draft_sources(&content, &inflight.sources(&resource_id));

    let (fm, _) = crate::ire::frontmatter::parse(&content);
    let title = fm
        .as_ref()
        .and_then(|m| m.get("title"))
        .map(|t| sanitize_resource_filename(t))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| resource_id[..8.min(resource_id.len())].to_string());

    let ire_path = format!("resources/{title}.md");
    let store = IreStore::new(workspace_path.clone());
    store
        .write_resource(&ire_path, &content, &app)
        .map_err(|e| e.to_string())?;

    cleanup_resource_cache(&workspace_path, &resource_id);
    let _ = fs::remove_file(&draft_path);
    inflight.remove(&resource_id);

    Ok(())
}

#[tauri::command]
pub fn save_resource_draft(
    active: State<'_, ActiveWorkspace>,
    resource_id: String,
    content: String,
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
    fs::write(&draft_path, content.as_bytes()).map_err(|e| e.to_string())
}

fn sanitize_resource_filename(title: &str) -> String {
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

/// Inject the in-flight `source_refs` into the draft's frontmatter (overwriting
/// any placeholder `sources:`). A no-op if there are no recorded sources.
fn normalize_resource_draft_sources(content: &str, source_refs: &[String]) -> String {
    if source_refs.is_empty() {
        return content.to_string();
    }
    replace_frontmatter_sources(content, source_refs)
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

/// Discard a resource. For an in-flight ingestion (id present in the registry):
/// drop the registry entry and clean its cache + draft. Otherwise the id is a
/// confirmed resource's file path (`resources/<slug>.md`): delete the file,
/// regenerate the index, and emit `resource-deleted`.
#[tauri::command]
pub fn discard_resource(
    app: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    inflight: State<'_, InflightResources>,
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

    if inflight.remove(&resource_id).is_some() {
        cleanup_resource_cache(&workspace_path, &resource_id);
        let draft_path = workspace_path
            .join(".ire/cache")
            .join(format!("{resource_id}_draft.md"));
        let _ = fs::remove_file(&draft_path);
        return Ok(());
    }

    // Confirmed resource: id is its file path.
    let store = IreStore::new(workspace_path.clone());
    store
        .delete_resource(&resource_id, &app)
        .map_err(|e| e.to_string())
}

/// Simulated ingestion for the `resource.add` MCP tool: the agent supplies the
/// markdown directly (no fetch). Registers the in-flight entry, writes the draft,
/// and opens an Approve/Discard preview tab (already in the "ready" state since
/// there is no summarization turn to wait on).
pub fn add_resource_from_markdown(
    app: &AppHandle,
    workspace_root: &Path,
    inflight: &InflightResources,
    markdown: &str,
    _title: Option<&str>,
    sources: &[String],
) -> anyhow::Result<String> {
    let resource_id = uuid::Uuid::new_v4().to_string();
    let draft_path = workspace_root
        .join(".ire/cache")
        .join(format!("{resource_id}_draft.md"));
    crate::ire::store::atomic_write(&draft_path, markdown)?;
    inflight.register(&resource_id, sources.to_vec());

    app.emit(
        "resource-pending",
        serde_json::json!({
            "resource_id": resource_id,
            "resource_status": "ready",
        }),
    )?;
    Ok(resource_id)
}

fn build_resource_system_prompt(workspace_root: &Path) -> String {
    let ire_root = workspace_root.join(".ire");
    let store = IreStore::new(workspace_root.to_path_buf());
    let mut parts: Vec<String> = Vec::new();

    if let Ok(content) = fs::read_to_string(ire_root.join("_SYSTEM.md")) {
        if !content.trim().is_empty() {
            parts.push(content);
        }
    }

    parts.push(prompts::resource_summarizer().to_string());

    if let Ok(ire) = store.read_ire() {
        let focus = focus_prompt_block(&ire.focus);
        if !focus.is_empty() {
            parts.push(focus);
        }
    }

    if let Ok(content) = fs::read_to_string(store.resources_dir.join("_index.md")) {
        if !content.trim().is_empty() {
            parts.push(format!("### resources/_index.md\n\n{content}"));
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
