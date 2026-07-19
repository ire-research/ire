use std::fs;
use std::path::Path;

use anyhow::Result;

use super::frontmatter;

/// Build `resources/_index.md`: one bullet per `resources/*.md` file (excluding
/// `_index.md` and dotfiles), sorted by filename. Returns the markdown body.
pub fn build_resources(resources_dir: &Path) -> Result<String> {
    let Ok(read_dir) = fs::read_dir(resources_dir) else {
        return Ok(String::new());
    };

    let mut entries: Vec<(String, String, String)> = vec![]; // (filename, title, summary)
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap_or_default();
        let (fm, body) = frontmatter::parse(&content);
        let title = fm
            .as_ref()
            .and_then(|m| m.get("title"))
            .map(|t| unquote(t))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| name.trim_end_matches(".md").to_string());
        let summary = fm
            .as_ref()
            .and_then(|m| m.get("TL;DR").or_else(|| m.get("summary")))
            .map(|s| unquote(s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| first_paragraph(body).unwrap_or_default());
        entries.push((name, title, summary));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    for (name, title, summary) in entries {
        if summary.is_empty() {
            out.push_str(&format!("- [{title}](./{name})\n"));
        } else {
            out.push_str(&format!("- [{title}](./{name}) — {summary}\n"));
        }
    }
    Ok(out)
}

/// Build `claims/_index.md`: one bullet per `claims/*.md` file (excluding
/// `_index.md` and dotfiles), sorted by filename.
pub fn build_claims(claims_dir: &Path) -> Result<String> {
    let Ok(read_dir) = fs::read_dir(claims_dir) else {
        return Ok(String::new());
    };

    let mut entries: Vec<(String, String, String)> = vec![]; // (filename, status, statement)
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap_or_default();
        let (fm, body) = frontmatter::parse(&content);
        let status = fm
            .as_ref()
            .and_then(|m| m.get("status"))
            .map(|s| unquote(s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "proposed".to_string());
        let statement = first_paragraph(body).unwrap_or_default();
        entries.push((name, status, statement));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    for (name, status, statement) in entries {
        let id = name.trim_end_matches(".md");
        if statement.is_empty() {
            out.push_str(&format!("- [{id}](./{name}) — {status}\n"));
        } else {
            out.push_str(&format!("- [{id}](./{name}) — {status} — {statement}\n"));
        }
    }
    Ok(out)
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').trim().to_string()
}

fn first_paragraph(text: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        if !t.is_empty() && !t.starts_with('#') {
            let s = if t.len() > 160 { &t[..160] } else { t };
            return Some(s.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_claims_reads_status_and_statement() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("head-not-bottleneck.md"),
            "---\ntype: Claim\nid: head-not-bottleneck\nstatus: supported\n---\n\nThe head is not the bottleneck.\n",
        )
        .unwrap();
        let out = build_claims(dir.path()).unwrap();
        assert_eq!(
            out,
            "- [head-not-bottleneck](./head-not-bottleneck.md) — supported — The head is not the bottleneck.\n"
        );
    }

    #[test]
    fn build_claims_defaults_status_to_proposed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("open-question.md"), "no frontmatter\n").unwrap();
        let out = build_claims(dir.path()).unwrap();
        assert!(out.starts_with("- [open-question](./open-question.md) — proposed"));
    }
}
