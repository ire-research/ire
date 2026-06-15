use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::json;
use tauri::AppHandle;
use uuid::Uuid;

use super::frontmatter;
use super::index;
use crate::db::models;
use crate::events;

pub struct WikiStore {
    pub wiki_root: PathBuf,
    pub ire_dir: PathBuf,
}

impl WikiStore {
    pub fn new(workspace_root: PathBuf) -> Self {
        let ire_dir = workspace_root.join(".ire");
        Self {
            wiki_root: ire_dir.join("wiki"),
            ire_dir,
        }
    }

    pub fn read(
        &self,
        rel_path: &str,
    ) -> Result<(String, Option<std::collections::HashMap<String, String>>)> {
        let path = self.wiki_root.join(rel_path);
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let (fm, _) = frontmatter::parse(&content);
        Ok((content, fm))
    }

    /// Atomically write `rel_path`, regenerate `_index.md`, and dispatch the
    /// matching `workspace-event` variant based on the path.
    pub fn write(&self, rel_path: &str, content: &str, app: &AppHandle) -> Result<()> {
        let path = self.wiki_root.join(rel_path);
        atomic_write(&path, content)?;

        let index_content = index::build(&self.wiki_root)?;
        atomic_write(&self.wiki_root.join("_index.md"), &index_content)?;

        self.dispatch_event(rel_path, content, app);
        Ok(())
    }

    /// Remove `rel_path` from the wiki and regenerate `_index.md`.
    pub fn delete(&self, rel_path: &str) -> Result<()> {
        let path = self.wiki_root.join(rel_path);
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
        let index_content = index::build(&self.wiki_root)?;
        atomic_write(&self.wiki_root.join("_index.md"), &index_content)?;
        Ok(())
    }

    /// Atomically rename `from` to `to` inside the wiki, update `_index.md`,
    /// and dispatch the matching `workspace-event` variant for `to`.
    pub fn rename(&self, from: &str, to: &str, app: &AppHandle) -> Result<()> {
        let src = self.wiki_root.join(from);
        let dst = self.wiki_root.join(to);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        fs::rename(&src, &dst).with_context(|| format!("rename {from} -> {to}"))?;

        let index_content = index::build(&self.wiki_root)?;
        atomic_write(&self.wiki_root.join("_index.md"), &index_content)?;

        let new_content = fs::read_to_string(&dst).unwrap_or_default();
        self.dispatch_event(to, &new_content, app);
        Ok(())
    }

    fn dispatch_event(&self, rel_path: &str, content: &str, app: &AppHandle) {
        tracing::info!(path = %rel_path, "workspace-event dispatch");
        match rel_path {
            "pulse.json" => {
                let parsed: serde_json::Value =
                    serde_json::from_str(content).unwrap_or_else(|_| json!({}));
                let rq = parsed
                    .get("research_question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let tw = parsed
                    .get("this_week")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                events::emit_pulse_changed(app, events::EventSource::Mutation, rq, tw);
            }
            "notes.md" => {
                events::emit_notes_changed(app, events::EventSource::Mutation, content);
            }
            "ideas.json" => {
                let parsed: serde_json::Value =
                    serde_json::from_str(content).unwrap_or_else(|_| json!([]));
                events::emit_ideas_changed(app, events::EventSource::Mutation, &parsed);
            }
            _ if rel_path.starts_with("resources/") && rel_path.ends_with(".md") => {
                self.index_resource(rel_path, content, app);
            }
            _ => {
                tracing::info!(path = %rel_path, "workspace-event: path did not match any variant");
            }
        }
    }

    /// Link a resources/*.md file back to its DB row(s) via the `sources` frontmatter
    /// array, then emit one `resource-changed` event per linked row.
    ///
    /// Match semantics mirror the previous post-turn `index_resource` command: a DB
    /// row matches when every source ref stored in its `url` column (single string
    /// for single-source resources, JSON-encoded array for `submit_resources`
    /// batches) appears in the file's `sources:` frontmatter.
    fn index_resource(&self, rel_path: &str, content: &str, app: &AppHandle) {
        let (fm, _) = frontmatter::parse(content);
        let Some(fm) = fm else {
            tracing::warn!(path = %rel_path, "index_resource: no frontmatter parsed");
            return;
        };
        let Some(sources_raw) = fm.get("sources") else {
            tracing::warn!(path = %rel_path, fm_keys = ?fm.keys().collect::<Vec<_>>(), "index_resource: no `sources:` key in frontmatter");
            return;
        };

        let file_sources: Vec<&str> = parse_sources_array(sources_raw);
        tracing::info!(path = %rel_path, sources = ?file_sources, "index_resource: parsed sources");

        let candidates = match models::list_unindexed_resources(&self.ire_dir) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "index_resource: list_unindexed_resources failed");
                return;
            }
        };

        let title = extract_title_from(content, rel_path);
        let mut linked = 0usize;
        for row in candidates {
            let stored = stored_source_refs(&row.url);
            if stored.is_empty() || !stored.iter().all(|s| file_sources.contains(&s.as_str())) {
                continue;
            }
            if let Err(e) =
                models::update_resource_indexed(&self.ire_dir, &row.url_sha256, rel_path, &title)
            {
                tracing::warn!(resource_id = %row.url_sha256, error = %e, "index_resource: update_resource_indexed failed");
                continue;
            }
            let source_label = row.source_label.clone().unwrap_or_else(|| row.url.clone());
            let payload = json!({
                "resource_id": row.url_sha256,
                "url": row.url,
                "source_type": row.source_type,
                "source_label": source_label,
                "title": title,
                "wiki_path": rel_path,
            });
            events::emit_resource_changed(app, events::EventSource::Mutation, &payload);
            linked += 1;
        }
        if linked == 0 {
            tracing::warn!(path = %rel_path, "index_resource: no unindexed DB row matched the file's sources");
        } else {
            tracing::info!(path = %rel_path, linked = linked, "index_resource: done");
        }
    }
}

fn parse_sources_array(value: &str) -> Vec<&str> {
    if let Ok(refs) = serde_json::from_str::<Vec<&str>>(value.trim()) {
        return refs;
    }

    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches(|c: char| c == '"' || c == '\''))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse a DB row's `url` column into its constituent source refs. Multi-source
/// batches store a JSON-encoded array; single-source rows store the ref directly.
fn stored_source_refs(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_else(|_| vec![value.to_string()])
}

fn extract_title_from(content: &str, rel_path: &str) -> String {
    let (fm, body) = frontmatter::parse(content);
    if let Some(fm) = fm {
        if let Some(t) = fm.get("title") {
            let t = t.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            let h = heading.trim();
            if !h.is_empty() {
                return h.to_string();
            }
        }
    }
    Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("resource")
        .to_string()
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
    use super::{parse_sources_array, stored_source_refs};

    #[test]
    fn stored_source_refs_reads_batch_json() {
        let refs = stored_source_refs(r#"["https://example.com/a","file:abc:paper.pdf"]"#);
        assert_eq!(refs, vec!["https://example.com/a", "file:abc:paper.pdf"]);
    }

    #[test]
    fn stored_source_refs_keeps_legacy_single_source() {
        let refs = stored_source_refs("https://example.com/a");
        assert_eq!(refs, vec!["https://example.com/a"]);
    }

    #[test]
    fn parse_sources_array_handles_inline_frontmatter() {
        let refs = parse_sources_array(r#"[https://example.com/a, "file:abc:paper.pdf"]"#);
        assert_eq!(refs, vec!["https://example.com/a", "file:abc:paper.pdf"]);
    }

    #[test]
    fn parse_sources_array_handles_block_frontmatter() {
        let content =
            "---\nsources:\n  - /Users/me/Documents/paper.pdf\n  - https://example.com/a\n---\n";
        let (fm, _) = crate::wiki::frontmatter::parse(content);
        let fm = fm.unwrap();
        let refs = parse_sources_array(fm.get("sources").unwrap());
        assert_eq!(
            refs,
            vec!["/Users/me/Documents/paper.pdf", "https://example.com/a"]
        );
    }
}
