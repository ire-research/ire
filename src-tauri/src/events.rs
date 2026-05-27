use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};

const CHANNEL: &str = "workspace-event";

/// Why a `workspace-event` was emitted. Carried inline on every payload so
/// side-effect listeners (e.g. panel-flash animations) can distinguish the
/// initial state burst on workspace open from live mutations.
#[derive(Clone, Copy)]
pub enum EventSource {
    Hydrate,
    Mutation,
}

impl EventSource {
    fn as_str(self) -> &'static str {
        match self {
            EventSource::Hydrate => "hydrate",
            EventSource::Mutation => "mutation",
        }
    }
}

pub fn emit_pulse_changed(
    app: &AppHandle,
    source: EventSource,
    research_question: &str,
    this_week: &str,
) {
    let _ = app.emit(
        CHANNEL,
        json!({
            "kind": "pulse-changed",
            "source": source.as_str(),
            "research_question": research_question,
            "this_week": this_week,
        }),
    );
}

pub fn emit_notes_changed(app: &AppHandle, source: EventSource, content: &str) {
    let _ = app.emit(
        CHANNEL,
        json!({
            "kind": "notes-changed",
            "source": source.as_str(),
            "content": content,
        }),
    );
}

pub fn emit_ideas_changed(app: &AppHandle, source: EventSource, ideas: &serde_json::Value) {
    let _ = app.emit(
        CHANNEL,
        json!({
            "kind": "ideas-changed",
            "source": source.as_str(),
            "ideas": ideas,
        }),
    );
}

pub fn emit_resource_changed<T: Serialize>(app: &AppHandle, source: EventSource, resource: &T) {
    let _ = app.emit(
        CHANNEL,
        json!({
            "kind": "resource-changed",
            "source": source.as_str(),
            "resource": resource,
        }),
    );
}

pub fn emit_experiment_changed<T: Serialize>(app: &AppHandle, source: EventSource, experiment: &T) {
    let _ = app.emit(
        CHANNEL,
        json!({
            "kind": "experiment-changed",
            "source": source.as_str(),
            "experiment": experiment,
        }),
    );
}

pub fn emit_experiment_deleted(app: &AppHandle, uuid: &str) {
    let _ = app.emit(
        CHANNEL,
        json!({
            "kind": "experiment-deleted",
            "source": EventSource::Mutation.as_str(),
            "uuid": uuid,
        }),
    );
}

pub fn emit_resource_deleted(app: &AppHandle, resource_id: &str) {
    let _ = app.emit(
        CHANNEL,
        json!({
            "kind": "resource-deleted",
            "source": EventSource::Mutation.as_str(),
            "resource_id": resource_id,
        }),
    );
}
