use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::wiki::WikiStore;
use crate::workspace::state::ActiveWorkspace;

#[derive(Debug, Serialize)]
pub struct WikiFileResult {
    pub content: String,
    pub frontmatter: Option<HashMap<String, String>>,
}

fn trunc(s: &str) -> &str {
    let end = s.char_indices().nth(80).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}

fn wiki_store(active: &State<'_, ActiveWorkspace>) -> Result<WikiStore, String> {
    let guard = active.0.lock().map_err(|e| e.to_string())?;
    let handle = guard.as_ref().ok_or("no workspace open")?;
    Ok(WikiStore::new(handle.state.path.clone()))
}

#[tauri::command]
pub fn read_wiki_file(
    path: String,
    active: State<'_, ActiveWorkspace>,
) -> Result<WikiFileResult, String> {
    tracing::debug!(path = %path, "read_wiki_file");
    let store = wiki_store(&active)?;
    let result = store.read(&path).map_err(|e| e.to_string());
    match &result {
        Ok(_) => tracing::debug!(path = %path, "wiki file read"),
        Err(e) => tracing::warn!(path = %path, error = %e, "read_wiki_file failed"),
    }
    result.map(|(content, frontmatter)| WikiFileResult { content, frontmatter })
}

#[tauri::command]
pub fn save_notes(
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(bytes = content.len(), preview = %trunc(&content), "save_notes");
    let store = wiki_store(&active)?;
    let result = store.write("notes.md", &content, &app).map_err(|e| e.to_string());
    match &result {
        Ok(_) => tracing::info!("notes.md saved"),
        Err(e) => tracing::warn!(error = %e, "save_notes failed"),
    }
    result
}

#[tauri::command]
pub fn save_ideas(
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(bytes = content.len(), preview = %trunc(&content), "save_ideas");
    let store = wiki_store(&active)?;
    let result = store.write("ideas.md", &content, &app).map_err(|e| e.to_string());
    match &result {
        Ok(_) => tracing::info!("ideas.md saved"),
        Err(e) => tracing::warn!(error = %e, "save_ideas failed"),
    }
    result
}
