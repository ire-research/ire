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
