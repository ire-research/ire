use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use super::frontmatter;
use super::index;

pub struct WikiStore {
    pub wiki_root: PathBuf,
}

impl WikiStore {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            wiki_root: workspace_root.join(".ire/wiki"),
        }
    }

    pub fn read(&self, rel_path: &str) -> Result<(String, Option<std::collections::HashMap<String, String>>)> {
        let path = self.wiki_root.join(rel_path);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let (fm, _) = frontmatter::parse(&content);
        Ok((content, fm))
    }

    /// Atomically write `rel_path` (relative to wiki_root), regenerate `_index.md`,
    /// append to `log.md`, and emit `wiki-changed`.
    pub fn write(&self, rel_path: &str, content: &str, app: &AppHandle) -> Result<()> {
        let path = self.wiki_root.join(rel_path);
        atomic_write(&path, content)?;

        let index_content = index::build(&self.wiki_root)?;
        atomic_write(&self.wiki_root.join("_index.md"), &index_content)?;

        let now = Local::now().format("%Y-%m-%d %H:%M");
        let log_entry = format!("## [{now}] write | {rel_path}\n");
        append_log(&self.wiki_root, &log_entry)?;

        let _ = app.emit("wiki-changed", serde_json::json!({ "path": rel_path }));
        Ok(())
    }
}

fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or(path);
    fs::create_dir_all(parent)
        .with_context(|| format!("create dir {}", parent.display()))?;

    let tmp = parent.join(format!("{}.tmp", Uuid::new_v4()));
    let mut f = fs::File::create(&tmp)
        .with_context(|| format!("create tmp {}", tmp.display()))?;
    f.write_all(content.as_bytes())?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, path)
        .with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}

fn append_log(wiki_root: &Path, entry: &str) -> Result<()> {
    let log_path = wiki_root.join("log.md");
    let existing = fs::read_to_string(&log_path).unwrap_or_default();
    atomic_write(&log_path, &(existing + entry))
}
