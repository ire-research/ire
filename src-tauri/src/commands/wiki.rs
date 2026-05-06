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
    let first_50: String = content.chars().take(50).collect();
    store.user_commit(&["notes.md"], &format!("notes: {}", first_50.trim()));
    tracing::info!("notes.md saved and committed");
    Ok(())
}

#[tauri::command]
pub fn save_ideas(
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(bytes = content.len(), "save_ideas");
    let store = wiki_store(&active)?;
    store.write("ideas.md", &content, &app).map_err(|e| e.to_string())?;
    let first_50: String = content.chars().take(50).collect();
    store.user_commit(&["ideas.md"], &format!("ideas: {}", first_50.trim()));
    tracing::info!("ideas.md saved and committed");
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
    store.write(&path, &content, &app).map_err(|e| e.to_string())?;
    let first_50: String = content.chars().take(50).collect();
    store.user_commit(&[&path], &format!("wiki: {}", first_50.trim()));
    Ok(())
}

#[tauri::command]
pub fn update_pulse_focus(
    focus: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(focus = %focus, "update_pulse_focus");
    let store = wiki_store(&active)?;
    let (current, _) = store
        .read("status/pulse.md")
        .map_err(|e| e.to_string())?;
    let updated = replace_focus_line(&current, &focus);
    store
        .write("status/pulse.md", &updated, &app)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Replace (or append) the `**Focus:** ...` line in pulse.md.
fn replace_focus_line(content: &str, focus: &str) -> String {
    let new_line = format!("**Focus:** {}", focus.trim());
    let mut found = false;
    let mut out: Vec<String> = content
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("**Focus:**") {
                found = true;
                new_line.clone()
            } else {
                l.to_string()
            }
        })
        .collect();
    if !found {
        if !out.is_empty() && !out.last().unwrap().is_empty() {
            out.push(String::new());
        }
        out.push(new_line);
    }
    let mut joined = out.join("\n");
    if content.ends_with('\n') && !joined.ends_with('\n') {
        joined.push('\n');
    }
    joined
}
