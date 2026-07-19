use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use uuid::Uuid;

use super::frontmatter;
use super::index;
use crate::events;

/// Serializes every in-process read-modify-write cycle on `ire.json` so the UI
/// setters and the experiment runner can't clobber each other's updates.
static IRE_LOCK: Mutex<()> = Mutex::new(());

// ── ire.json schema ─────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IreFocus {
    #[serde(default)]
    pub research_question: String,
    #[serde(default)]
    pub this_week: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IreIdea {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IreExperiment {
    pub uuid: String,
    pub name: String,
    pub command: String,
    pub status: String,
    pub started_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IreContent {
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub focus: IreFocus,
    #[serde(default)]
    pub ideas: Vec<IreIdea>,
    #[serde(default)]
    pub experiments: Vec<IreExperiment>,
}

/// The state record rooted at `.ire/`. Owns `ire.json` (notes/focus/ideas/
/// experiments) and the file-based `resources/` and `claims/` trees.
pub struct IreStore {
    pub ire_dir: PathBuf,
    pub resources_dir: PathBuf,
    pub claims_dir: PathBuf,
}

impl IreStore {
    pub fn new(workspace_root: PathBuf) -> Self {
        let ire_dir = workspace_root.join(".ire");
        let resources_dir = ire_dir.join("resources");
        let claims_dir = ire_dir.join("claims");
        Self {
            ire_dir,
            resources_dir,
            claims_dir,
        }
    }

    fn ire_path(&self) -> PathBuf {
        self.ire_dir.join("ire.json")
    }

    /// Read a resource markdown file relative to `.ire/resources/`. Returns content and parsed frontmatter.
    pub fn read_resource(&self, rel_path: &str) -> Result<(String, Option<HashMap<String, String>>)> {
        let path = self.ire_dir.join(rel_path);
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let (fm, _) = frontmatter::parse(&content);
        Ok((content, fm))
    }

    // ── ire.json ────────────────────────────────────────────────────────────

    /// Parse `ire.json` into a typed record. A missing or empty file yields
    /// defaults.
    pub fn read_ire(&self) -> Result<IreContent> {
        let (raw, _) = self.read_ire_raw();
        if raw.trim().is_empty() {
            return Ok(IreContent::default());
        }
        serde_json::from_str(&raw).context("parse ire.json")
    }

    /// Raw `ire.json` bytes plus a content-hash version token. A missing file
    /// yields an empty string and the hash of empty.
    pub fn read_ire_raw(&self) -> (String, String) {
        let raw = fs::read_to_string(self.ire_path()).unwrap_or_default();
        let version = hash(&raw);
        (raw, version)
    }

    /// Read-modify-write `ire.json` under the in-process lock, then emit the
    /// notes/focus/ideas section events.
    pub fn update_ire(&self, app: &AppHandle, mutate: impl FnOnce(&mut IreContent)) -> Result<()> {
        let _guard = IRE_LOCK.lock().unwrap();
        let mut content = self.read_ire()?;
        mutate(&mut content);
        self.persist(&content)?;
        emit_sections(app, &content);
        Ok(())
    }

    /// String-replacement edit of `ire.json`, mirroring the built-in `Edit` tool.
    /// Requires a fresh `version` (the content hash returned by the read the
    /// agent must perform first). Fails on a stale version, a missing `old`, or a
    /// non-unique `old`. On success it shape-validates, writes, and emits events.
    pub fn edit_ire(&self, old: &str, new: &str, version: &str, app: &AppHandle) -> Result<()> {
        let _guard = IRE_LOCK.lock().unwrap();
        let (raw, current) = self.read_ire_raw();
        let content = apply_edit(&raw, &current, old, new, version)?;
        self.persist(&content)?;
        emit_sections(app, &content);
        Ok(())
    }

    /// Insert or replace one experiment in `ire.json` (matched by uuid) without
    /// emitting section events — `experiment-changed` is owned by the runner.
    pub fn upsert_experiment(&self, exp: IreExperiment) -> Result<()> {
        let _guard = IRE_LOCK.lock().unwrap();
        let mut content = self.read_ire()?;
        match content.experiments.iter_mut().find(|e| e.uuid == exp.uuid) {
            Some(slot) => *slot = exp,
            None => content.experiments.insert(0, exp),
        }
        self.persist(&content)
    }

    /// Remove one experiment from `ire.json` by uuid.
    pub fn remove_experiment(&self, uuid: &str) -> Result<()> {
        let _guard = IRE_LOCK.lock().unwrap();
        let mut content = self.read_ire()?;
        content.experiments.retain(|e| e.uuid != uuid);
        self.persist(&content)
    }

    fn persist(&self, content: &IreContent) -> Result<()> {
        let text = serde_json::to_string_pretty(content)? + "\n";
        atomic_write(&self.ire_path(), &text)
    }

    // ── resources ─────────────────────────────────────────────────────────────

    /// Atomically write a `resources/<slug>.md` file, regenerate
    /// `resources/_index.md`, and emit `resource-changed` derived from the file's
    /// frontmatter (title + sources).
    pub fn write_resource(&self, rel_path: &str, content: &str, app: &AppHandle) -> Result<()> {
        atomic_write(&self.ire_dir.join(rel_path), content)?;
        self.rebuild_resource_index()?;
        let (title, sources) = resource_meta(content, rel_path);
        events::emit_resource_changed(
            app,
            events::EventSource::Mutation,
            &serde_json::json!({ "path": rel_path, "title": title, "sources": sources }),
        );
        Ok(())
    }

    /// Delete a `resources/<slug>.md` file, regenerate the index, and emit
    /// `resource-deleted`.
    pub fn delete_resource(&self, rel_path: &str, app: &AppHandle) -> Result<()> {
        let path = self.ire_dir.join(rel_path);
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
        self.rebuild_resource_index()?;
        events::emit_resource_deleted(app, rel_path);
        Ok(())
    }

    pub fn rebuild_resource_index(&self) -> Result<()> {
        let content = index::build_resources(&self.resources_dir)?;
        atomic_write(&self.resources_dir.join("_index.md"), &content)
    }

    /// Scan `resources/*.md` and return `{ path, title, sources }` for each,
    /// sorted by filename. Used for the open-workspace hydration burst.
    pub fn list_resources(&self) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let Ok(read_dir) = fs::read_dir(&self.resources_dir) else {
            return out;
        };
        let mut paths: Vec<PathBuf> = read_dir
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| !n.starts_with('_') && !n.starts_with('.'))
                    .unwrap_or(false)
            })
            .collect();
        paths.sort();
        for path in paths {
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let rel = format!("resources/{name}");
            let (title, sources) = resource_meta(&content, &rel);
            out.push(serde_json::json!({ "path": rel, "title": title, "sources": sources }));
        }
        out
    }

    // ── claims ────────────────────────────────────────────────────────────────

    /// Atomically write a `claims/<id>.md` file and regenerate `claims/_index.md`.
    pub fn write_claim(&self, rel_path: &str, content: &str) -> Result<()> {
        atomic_write(&self.ire_dir.join(rel_path), content)?;
        self.rebuild_claims_index()
    }

    pub fn rebuild_claims_index(&self) -> Result<()> {
        let content = index::build_claims(&self.claims_dir)?;
        atomic_write(&self.claims_dir.join("_index.md"), &content)
    }
}

/// Render the focus section for injection into an agent's system prompt. Empty
/// if no focus is set.
pub fn focus_prompt_block(focus: &IreFocus) -> String {
    let rq = focus.research_question.trim();
    let tw = focus.this_week.trim();
    if rq.is_empty() && tw.is_empty() {
        return String::new();
    }
    let mut out = String::from("### Focus\n\n");
    if !rq.is_empty() {
        out.push_str(&format!("Research question: {rq}\n"));
    }
    if !tw.is_empty() {
        out.push_str(&format!("This week: {tw}\n"));
    }
    out
}

/// Pure core of `edit_ire`: validate the version, apply a unique string
/// replacement, and re-parse against the schema.
fn apply_edit(
    raw: &str,
    current_version: &str,
    old: &str,
    new: &str,
    version: &str,
) -> Result<IreContent> {
    if version != current_version {
        return Err(anyhow!(
            "ire.json changed since you last read it (or you didn't read it first); re-read with ire.read and retry"
        ));
    }
    let count = raw.matches(old).count();
    if count == 0 {
        return Err(anyhow!("`old` text not found in ire.json"));
    }
    if count > 1 {
        return Err(anyhow!(
            "`old` text is not unique in ire.json ({count} matches) — include more surrounding context"
        ));
    }
    let updated = raw.replacen(old, new, 1);
    serde_json::from_str(&updated).context("the edited ire.json is not valid against the schema")
}

fn emit_sections(app: &AppHandle, content: &IreContent) {
    events::emit_notes_changed(app, events::EventSource::Mutation, &content.notes);
    events::emit_focus_changed(
        app,
        events::EventSource::Mutation,
        &content.focus.research_question,
        &content.focus.this_week,
    );
    let ideas = serde_json::to_value(&content.ideas).unwrap_or_else(|_| serde_json::json!([]));
    events::emit_ideas_changed(app, events::EventSource::Mutation, &ideas);
}

fn hash(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Extract `(title, sources)` from a resource file's frontmatter, with
/// fallbacks: title → first `# ` heading → filename; sources → empty.
fn resource_meta(content: &str, rel_path: &str) -> (String, Vec<String>) {
    let (fm, body) = frontmatter::parse(content);
    let title = fm
        .as_ref()
        .and_then(|m| m.get("title"))
        .map(|t| unquote(t))
        .filter(|t| !t.is_empty())
        .or_else(|| {
            body.lines()
                .find_map(|l| l.strip_prefix("# ").map(|h| h.trim().to_string()))
                .filter(|h| !h.is_empty())
        })
        .unwrap_or_else(|| {
            Path::new(rel_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("resource")
                .to_string()
        });
    let sources = fm
        .as_ref()
        .and_then(|m| m.get("sources"))
        .map(|s| parse_sources(s))
        .unwrap_or_default();
    (title, sources)
}

/// Frontmatter list values are stored JSON-encoded by `frontmatter::parse`;
/// fall back to a single scalar source.
fn parse_sources(value: &str) -> Vec<String> {
    if let Ok(v) = serde_json::from_str::<Vec<String>>(value.trim()) {
        return v;
    }
    let t = unquote(value);
    if t.is_empty() {
        vec![]
    } else {
        vec![t]
    }
}

/// Strip surrounding double quotes from a scalar frontmatter value.
fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').trim().to_string()
}

pub(crate) fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or(path);
    fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;

    let tmp = parent.join(format!("{}.tmp", Uuid::new_v4()));
    let mut f = fs::File::create(&tmp).with_context(|| format!("create tmp {}", tmp.display()))?;
    f.write_all(content.as_bytes())?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, IreStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = IreStore::new(dir.path().to_path_buf());
        fs::create_dir_all(&store.resources_dir).unwrap();
        (dir, store)
    }

    #[test]
    fn read_ire_defaults_when_missing() {
        let (_d, s) = store();
        let c = s.read_ire().unwrap();
        assert!(c.notes.is_empty());
        assert!(c.ideas.is_empty());
    }

    #[test]
    fn apply_edit_rejects_stale_version() {
        let raw = "{\"notes\":\"a\"}\n";
        let current = hash(raw);
        let err = apply_edit(raw, &current, "a", "b", "deadbeef")
            .unwrap_err()
            .to_string();
        assert!(err.contains("changed since"), "{err}");
    }

    #[test]
    fn apply_edit_requires_unique_old() {
        let raw = "{\"notes\":\"x x\"}\n";
        let current = hash(raw);
        let err = apply_edit(raw, &current, "x", "y", &current)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not unique"), "{err}");
    }

    #[test]
    fn apply_edit_replaces_and_validates() {
        let raw = "{\"notes\":\"hello\",\"focus\":{\"research_question\":\"\",\"this_week\":\"\"},\"ideas\":[],\"experiments\":[]}\n";
        let current = hash(raw);
        let c = apply_edit(raw, &current, "hello", "world", &current).unwrap();
        assert_eq!(c.notes, "world");
    }

    #[test]
    fn resource_meta_reads_frontmatter() {
        let content =
            "---\ntitle: \"My Paper\"\nsources:\n  - https://example.com/a\n---\n\n# body\n";
        let (title, sources) = resource_meta(content, "resources/my-paper.md");
        assert_eq!(title, "My Paper");
        assert_eq!(sources, vec!["https://example.com/a"]);
    }
}
