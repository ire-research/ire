use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};

const SYSTEM_MD: &str = include_str!("../../assets/seed/_SYSTEM.md");
const IRE_JSON: &str = include_str!("../../assets/seed/ire.json");
const LONG_TERM_MD: &str = include_str!("../../assets/seed/long-term.md");

// Runtime artifacts (local.db, mcp.json, mcp.sock, .lock, workspace UI state)
// now live in `~/.ire/workspaces/<id>/`, so the only workspace-local thing to
// ignore is the resource/experiment scratch cache.
const GITIGNORE_ENTRIES: &[&str] = &[".ire/cache/"];

/// Validate that `path` is an existing IRE workspace.
pub fn validate_existing(path: &Path) -> Result<()> {
    if !path.is_dir() {
        return Err(anyhow!("not a directory: {}", path.display()));
    }
    let system = path.join(".ire/_SYSTEM.md");
    let ire_json = path.join(".ire/ire.json");
    if !system.exists() || !ire_json.exists() {
        return Err(anyhow!(
            "missing .ire/_SYSTEM.md or .ire/ire.json — not an IRE workspace"
        ));
    }
    Ok(())
}

/// Ensure the workspace has a git repository and IRE gitignore entries.
/// No-op if `.git` already exists.
pub fn ensure_git(path: &Path) -> Result<()> {
    if !path.join(".git").exists() {
        run_git(path, &["init", "--quiet"])?;
        ensure_gitignore(path)?;
    }
    Ok(())
}

/// Scaffold a fresh `.ire/` tree, seed wiki, gitignore, and git-init if needed.
/// Idempotent: existing files are not overwritten.
pub fn initialize(path: &Path) -> Result<()> {
    if !path.is_dir() {
        return Err(anyhow!("not a directory: {}", path.display()));
    }

    // git init if absent
    if !path.join(".git").exists() {
        run_git(path, &["init", "--quiet"])?;
    }

    let ire = path.join(".ire");
    let short_term = ire.join("short-term");
    let resources = ire.join("resources");
    let claims = ire.join("claims");
    let cache = ire.join("cache");

    for d in [&ire, &short_term, &resources, &claims, &cache] {
        fs::create_dir_all(d).with_context(|| format!("create {}", d.display()))?;
    }

    write_if_absent(&ire.join("_SYSTEM.md"), SYSTEM_MD)?;
    write_if_absent(&ire.join("ire.json"), IRE_JSON)?;
    write_if_absent(&ire.join("long-term.md"), LONG_TERM_MD)?;
    // resources/_index.md and claims/_index.md are regenerated whenever a
    // resource/claim is written; seed empty.
    write_if_absent(&resources.join("_index.md"), "")?;
    write_if_absent(&claims.join("_index.md"), "")?;

    ensure_gitignore(path)?;
    Ok(())
}

fn write_if_absent(p: &Path, content: &str) -> Result<()> {
    if p.exists() {
        return Ok(());
    }
    let mut f = fs::File::create(p).with_context(|| format!("create {}", p.display()))?;
    f.write_all(content.as_bytes())?;
    f.sync_all()?;
    Ok(())
}

fn ensure_gitignore(path: &Path) -> Result<()> {
    let gi = path.join(".gitignore");
    let existing = if gi.exists() {
        fs::read_to_string(&gi)?
    } else {
        String::new()
    };
    let mut out = existing.clone();
    let mut added = false;
    for entry in GITIGNORE_ENTRIES {
        if !line_present(&existing, entry) {
            if !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            if !added {
                out.push_str("\n# IRE\n");
                added = true;
            }
            out.push_str(entry);
            out.push('\n');
        }
    }
    if added || !gi.exists() {
        fs::write(&gi, out).with_context(|| format!("write {}", gi.display()))?;
    }
    Ok(())
}

fn line_present(haystack: &str, needle: &str) -> bool {
    haystack.lines().any(|l| l.trim() == needle)
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git").args(args).current_dir(cwd).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

/// Like [`home_data_dir`] but returns a `Result` with a consistent error
/// message, eliminating the repeated `.ok_or("cannot determine home directory")`
/// at every call site.
pub fn require_home_data_dir(path: &Path) -> Result<PathBuf, String> {
    home_data_dir(path).ok_or_else(|| "cannot determine home directory".to_string())
}

/// `~/.ire/workspaces/<sanitized-name>-<8-hex>/` for a workspace path — the
/// per-user home for runtime/local artifacts (local.db, mcp.json, mcp.sock,
/// .lock) that must not live in the git-tracked workspace `.ire/`.
pub fn home_data_dir(path: &Path) -> Option<PathBuf> {
    let home = crate::binary::home_dir()?;
    let hash = Sha256::digest(path.to_string_lossy().as_bytes());
    let id: String = hash.iter().take(4).map(|b| format!("{b:02x}")).collect();
    let name: String = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect();
    Some(home.join(".ire").join("workspaces").join(format!("{name}-{id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn initialize_scaffolds_full_tree() {
        if !git_available() {
            eprintln!("skipping: git not available");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        initialize(root).unwrap();

        for rel in [
            ".git",
            ".ire/_SYSTEM.md",
            ".ire/ire.json",
            ".ire/long-term.md",
            ".ire/short-term",
            ".ire/resources",
            ".ire/resources/_index.md",
            ".ire/claims",
            ".ire/claims/_index.md",
            ".ire/cache",
            ".gitignore",
        ] {
            assert!(root.join(rel).exists(), "missing {rel}");
        }

        for rel in [
            ".ire/wiki",
            ".ire/wiki/pulse.json",
            ".ire/wiki/notes.md",
            ".ire/wiki/experiments",
            ".ire/pulse.json",
            ".ire/notes.md",
        ] {
            assert!(!root.join(rel).exists(), "unexpected {rel}");
        }

        let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        for entry in GITIGNORE_ENTRIES {
            assert!(
                gitignore.lines().any(|l| l.trim() == *entry),
                "gitignore missing {entry}"
            );
        }

        validate_existing(root).unwrap();

        // Idempotent re-run.
        initialize(root).unwrap();

        // Custom user content survives re-init.
        let ire_json = root.join(".ire/ire.json");
        fs::write(&ire_json, "user data").unwrap();
        initialize(root).unwrap();
        assert_eq!(fs::read_to_string(&ire_json).unwrap(), "user data");
    }

    #[test]
    fn gitignore_dedupe_appends_only_missing_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join(".gitignore"), ".ire/cache/\nnode_modules\n").unwrap();
        ensure_gitignore(root).unwrap();
        let after = fs::read_to_string(root.join(".gitignore")).unwrap();
        // Pre-existing entry not duplicated.
        assert_eq!(after.matches(".ire/cache/").count(), 1);
        assert!(after.contains("node_modules"));
    }
}
