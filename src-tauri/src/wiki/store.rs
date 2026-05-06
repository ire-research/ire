use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use super::frontmatter;
use super::index;

pub struct WikiStore {
    pub wiki_root: PathBuf,
    pub workspace_root: PathBuf,
}

impl WikiStore {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            wiki_root: workspace_root.join(".ire/wiki"),
            workspace_root,
        }
    }

    pub fn read(&self, rel_path: &str) -> Result<(String, Option<std::collections::HashMap<String, String>>)> {
        let path = self.wiki_root.join(rel_path);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let (fm, _) = frontmatter::parse(&content);
        Ok((content, fm))
    }

    /// Atomically write `rel_path`, regenerate `_index.md`, emit `wiki-changed`,
    /// and auto-commit if the path is auto-tracked.
    pub fn write(&self, rel_path: &str, content: &str, app: &AppHandle) -> Result<()> {
        let path = self.wiki_root.join(rel_path);
        atomic_write(&path, content)?;

        let index_content = index::build(&self.wiki_root)?;
        atomic_write(&self.wiki_root.join("_index.md"), &index_content)?;

        if is_auto_tracked(rel_path) {
            let msg = format!("auto: wiki {rel_path}");
            self.git_auto_commit(&[rel_path, "_index.md"], &msg);
        }

        let _ = app.emit("wiki-changed", serde_json::json!({ "path": rel_path }));
        Ok(())
    }

    /// Atomically rename `from` to `to` inside the wiki, update `_index.md`,
    /// emit `wiki-changed`, and auto-commit if applicable.
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

        if is_auto_tracked(from) || is_auto_tracked(to) {
            let msg = format!("auto: wiki rename {from} -> {to}");
            self.git_auto_commit(&[to, "_index.md"], &msg);
        }

        let _ = app.emit("wiki-changed", serde_json::json!({ "path": to }));
        Ok(())
    }

    /// Commit user-tracked paths (notes.md, ideas.md) alongside `_index.md`.
    /// Failures are logged but not propagated.
    pub fn user_commit(&self, rel_wiki_paths: &[&str], message: &str) {
        let prefix = ".ire/wiki/";
        let mut add = Command::new("git");
        add.current_dir(&self.workspace_root).arg("add");
        for p in rel_wiki_paths {
            add.arg(format!("{prefix}{p}"));
        }
        add.arg(format!("{prefix}_index.md"));
        match add.output() {
            Ok(out) if !out.status.success() => {
                tracing::warn!(
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "git add failed (user commit)"
                );
                return;
            }
            Err(e) => {
                tracing::warn!(error = %e, "git add error (user commit)");
                return;
            }
            _ => {}
        }

        let commit_out = Command::new("git")
            .current_dir(&self.workspace_root)
            .args(["commit", "-m", message, "--quiet"])
            .output();
        match commit_out {
            Ok(out) if !out.status.success() => {
                tracing::warn!(
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "git user commit failed (nothing staged?)"
                );
            }
            Err(e) => tracing::warn!(error = %e, "git user commit error"),
            _ => {}
        }
    }

    /// Stage all files under `resources/` + `_index.md` and commit.
    /// Called after the Confirm flow writes a resource wiki page.
    pub fn commit_resource_files(&self) {
        let mut add = Command::new("git");
        add.current_dir(&self.workspace_root)
            .args(["add", ".ire/wiki/resources/", ".ire/wiki/_index.md"]);
        match add.output() {
            Ok(out) if !out.status.success() => {
                tracing::warn!(
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "git add resources failed"
                );
                return;
            }
            Err(e) => {
                tracing::warn!(error = %e, "git add resources error");
                return;
            }
            _ => {}
        }

        let commit_out = Command::new("git")
            .current_dir(&self.workspace_root)
            .args(["commit", "-m", "resources: add summary", "--quiet"])
            .output();
        match commit_out {
            Ok(out) if !out.status.success() => {
                tracing::warn!(
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "git commit resources failed (nothing staged?)"
                );
            }
            Err(e) => tracing::warn!(error = %e, "git commit resources error"),
            _ => {}
        }
    }

    /// Stage `rel_paths` (relative to wiki root) and commit. Failures are
    /// logged but do not propagate — the files are on disk and the next
    /// write will re-stage them.
    pub fn git_auto_commit(&self, rel_wiki_paths: &[&str], message: &str) {
        let prefix = ".ire/wiki/";
        let mut add = Command::new("git");
        add.current_dir(&self.workspace_root).arg("add");
        for p in rel_wiki_paths {
            add.arg(format!("{prefix}{p}"));
        }
        match add.output() {
            Ok(out) if !out.status.success() => {
                tracing::warn!(
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "git add failed"
                );
                return;
            }
            Err(e) => {
                tracing::warn!(error = %e, "git add error");
                return;
            }
            _ => {}
        }

        let commit_out = Command::new("git")
            .current_dir(&self.workspace_root)
            .args(["commit", "-m", message, "--quiet"])
            .output();
        match commit_out {
            Ok(out) if !out.status.success() => {
                tracing::warn!(
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "git commit failed (nothing staged?)"
                );
            }
            Err(e) => tracing::warn!(error = %e, "git commit error"),
            _ => {}
        }
    }
}

pub fn is_auto_tracked(rel_path: &str) -> bool {
    rel_path.starts_with("status/")
        || rel_path == "_schema.md"
        || rel_path == "_index.md"
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

