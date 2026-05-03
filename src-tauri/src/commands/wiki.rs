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
    let store = wiki_store(&active)?;
    let (content, frontmatter) = store.read(&path).map_err(|e| e.to_string())?;
    Ok(WikiFileResult { content, frontmatter })
}

#[tauri::command]
pub fn save_notes(
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = wiki_store(&active)?;
    store.write("notes.md", &content, &app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_ideas(
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = wiki_store(&active)?;
    store.write("ideas.md", &content, &app).map_err(|e| e.to_string())
}
