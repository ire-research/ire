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
    result.map(|(content, frontmatter)| WikiFileResult {
        content,
        frontmatter,
    })
}

#[tauri::command]
pub fn save_notes(
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(bytes = content.len(), "save_notes");
    let store = wiki_store(&active)?;
    store
        .write("notes.md", &content, &app)
        .map_err(|e| e.to_string())?;
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
    store
        .write(&path, &content, &app)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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
pub fn save_pulse_field(
    field: String,
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = wiki_store(&active)?;
    let mut pulse = read_pulse_content(&store).map_err(|e| e.to_string())?;
    match field.as_str() {
        "research_question" => pulse.research_question = content,
        "this_week" => pulse.this_week = content,
        _ => return Err(format!("unknown field: {field}")),
    };
    write_pulse_content(&store, &pulse, &app).map_err(|e| e.to_string())
}

pub(crate) fn read_pulse_content(store: &WikiStore) -> anyhow::Result<PulseContent> {
    let path = store.wiki_root.join("pulse.json");
    if !path.exists() {
        return Ok(PulseContent {
            research_question: String::new(),
            this_week: String::new(),
        });
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub(crate) fn write_pulse_content(
    store: &WikiStore,
    pulse: &PulseContent,
    app: &tauri::AppHandle,
) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(pulse)?;
    store.write("pulse.json", &format!("{json}\n"), app)
}

pub(crate) fn patch_pulse_content(
    mut pulse: PulseContent,
    research_question: Option<&str>,
    this_week: Option<&str>,
) -> PulseContent {
    if let Some(research_question) = research_question {
        pulse.research_question = research_question.to_string();
    }
    if let Some(this_week) = this_week {
        pulse.this_week = this_week.to_string();
    }
    pulse
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_pulse_content_preserves_unspecified_fields() {
        let pulse = PulseContent {
            research_question: "old question".to_string(),
            this_week: "old week".to_string(),
        };

        let updated = patch_pulse_content(pulse, Some("new question"), None);

        assert_eq!(
            updated,
            PulseContent {
                research_question: "new question".to_string(),
                this_week: "old week".to_string(),
            }
        );
    }
}

#[tauri::command]
pub fn save_ideas_json(
    ideas: Vec<IdeaItem>,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = wiki_store(&active)?;
    let json = serde_json::to_string_pretty(&ideas).map_err(|e| e.to_string())?;
    store
        .write("ideas.json", &json, &app)
        .map_err(|e| e.to_string())
}
