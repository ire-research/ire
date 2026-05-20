use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
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

    /// Atomically write `rel_path`, regenerate `_index.md`, and emit `wiki-changed`.
    pub fn write(&self, rel_path: &str, content: &str, app: &AppHandle) -> Result<()> {
        let path = self.wiki_root.join(rel_path);
        atomic_write(&path, content)?;

        let index_content = index::build(&self.wiki_root)?;
        atomic_write(&self.wiki_root.join("_index.md"), &index_content)?;

        let _ = app.emit("wiki-changed", serde_json::json!({ "path": rel_path }));
        Ok(())
    }

    /// Atomically rename `from` to `to` inside the wiki, update `_index.md`,
    /// and emit `wiki-changed`.
    pub fn rename(&self, from: &str, to: &str, app: &AppHandle) -> Result<()> {
        let src = self.wiki_root.join(from);
        let dst = self.wiki_root.join(to);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        fs::rename(&src, &dst)
            .with_context(|| format!("rename {from} -> {to}"))?;

        let index_content = index::build(&self.wiki_root)?;
        atomic_write(&self.wiki_root.join("_index.md"), &index_content)?;

        let _ = app.emit("wiki-changed", serde_json::json!({ "path": to }));
        Ok(())
    }
}

pub(crate) fn atomic_write(path: &Path, content: &str) -> Result<()> {
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
