use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::ire::{IreIdea, IreStore};
use crate::workspace::state::ActiveWorkspace;

#[derive(Debug, Serialize)]
pub struct IreFileResult {
    pub content: String,
    pub frontmatter: Option<HashMap<String, String>>,
}

fn ire_store(active: &State<'_, ActiveWorkspace>) -> Result<IreStore, String> {
    let guard = active.0.lock().map_err(|e| e.to_string())?;
    let handle = guard.as_ref().ok_or("no workspace open")?;
    Ok(IreStore::new(handle.state.path.clone()))
}

/// Read a resource markdown file (the resource preview pane reads `resources/*.md`).
#[tauri::command]
pub fn read_resource(
    path: String,
    active: State<'_, ActiveWorkspace>,
) -> Result<IreFileResult, String> {
    tracing::debug!(path = %path, "read_resource");
    if !path.starts_with("resources/") || !path.ends_with(".md") || path.contains("..") {
        return Err(format!("invalid resource path: {path}"));
    }
    let store = ire_store(&active)?;
    store
        .read_resource(&path)
        .map(|(content, frontmatter)| IreFileResult {
            content,
            frontmatter,
        })
        .map_err(|e| e.to_string())
}

/// Save a resource markdown file (the preview pane's inline editor).
#[tauri::command]
pub fn save_resource(
    path: String,
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(path = %path, bytes = content.len(), "save_resource");
    if !path.starts_with("resources/") || !path.ends_with(".md") || path.contains("..") {
        return Err(format!("invalid resource path: {path}"));
    }
    let store = ire_store(&active)?;
    store
        .write_resource(&path, &content, &app)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_notes(
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(bytes = content.len(), "save_notes");
    let store = ire_store(&active)?;
    store
        .update_ire(&app, |c| c.notes = content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_focus_field(
    field: String,
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = ire_store(&active)?;
    store
        .update_ire(&app, |c| match field.as_str() {
            "research_question" => c.focus.research_question = content,
            "this_week" => c.focus.this_week = content,
            other => tracing::warn!(field = %other, "save_focus_field: unknown field"),
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_ideas(
    ideas: Vec<IreIdea>,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = ire_store(&active)?;
    store
        .update_ire(&app, |c| c.ideas = ideas)
        .map_err(|e| e.to_string())
}
