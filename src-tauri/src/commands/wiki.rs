use std::collections::HashMap;

use serde::{Deserialize, Serialize};
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
    tracing::info!(bytes = content.len(), "save_notes");
    let store = wiki_store(&active)?;
    store.write("notes.md", &content, &app).map_err(|e| e.to_string())?;
    tracing::info!("notes.md saved");
    Ok(())
}

#[tauri::command]
pub fn save_wiki_file(
    path: String,
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(path = %path, bytes = content.len(), "save_wiki_file");
    let store = wiki_store(&active)?;
    store.write(&path, &content, &app).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct PulseContent {
    pub research_question: String,
    pub this_week: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdeaItem {
    pub id: String,
    pub text: String,
    pub trashed: bool,
    pub order: i64,
}

#[tauri::command]
pub fn read_pulse(active: State<'_, ActiveWorkspace>) -> Result<PulseContent, String> {
    let store = wiki_store(&active)?;
    let rq = store.read("pulse/RESEARCH-QUESTION.md")
        .map(|(c, _)| c)
        .unwrap_or_default();
    let tw = store.read("pulse/THIS-WEEK.md")
        .map(|(c, _)| c)
        .unwrap_or_default();
    Ok(PulseContent { research_question: rq, this_week: tw })
}

#[tauri::command]
pub fn save_pulse_field(
    field: String,
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let path = match field.as_str() {
        "research_question" => "pulse/RESEARCH-QUESTION.md",
        "this_week" => "pulse/THIS-WEEK.md",
        _ => return Err(format!("unknown field: {field}")),
    };
    let store = wiki_store(&active)?;
    store.write(path, &content, &app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_ideas(active: State<'_, ActiveWorkspace>) -> Result<Vec<IdeaItem>, String> {
    let store = wiki_store(&active)?;
    let path = store.wiki_root.join("ideas.json");
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_ideas_json(
    ideas: Vec<IdeaItem>,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = wiki_store(&active)?;
    let json = serde_json::to_string_pretty(&ideas).map_err(|e| e.to_string())?;
    store.write("ideas.json", &json, &app).map_err(|e| e.to_string())
}
