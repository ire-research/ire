use std::fs;
use std::path::Path;

use anyhow::Result;

use super::frontmatter;

pub fn build(wiki_root: &Path) -> Result<String> {
    let mut entries: Vec<(String, String, String)> = vec![];
    collect(wiki_root, wiki_root, &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    for (rel, display, summary) in entries {
        out.push_str(&format!("- [{display}](./{rel}) — {summary}\n"));
    }
    Ok(out)
}

fn collect(wiki_root: &Path, dir: &Path, out: &mut Vec<(String, String, String)>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(wiki_root, &path, out)?;
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("md" | "json")
        ) {
            let rel = path
                .strip_prefix(wiki_root)?
                .to_string_lossy()
                .replace('\\', "/");
            let summary = if path.extension().and_then(|e| e.to_str()) == Some("json") {
                json_summary(&name)
            } else {
                let content = fs::read_to_string(&path).unwrap_or_default();
                let (fm, body) = frontmatter::parse(&content);
                fm.as_ref()
                    .and_then(|m| m.get("summary"))
                    .cloned()
                    .unwrap_or_else(|| first_paragraph(body).unwrap_or_default())
            };
            let display = name
                .trim_end_matches(".md")
                .trim_end_matches(".json")
                .to_string();
            out.push((rel, display, summary));
        }
    }
    Ok(())
}

fn json_summary(name: &str) -> String {
    match name {
        "ideas.json" => "user ideas list".to_string(),
        "pulse.json" => "current research question and weekly focus".to_string(),
        _ => "structured data".to_string(),
    }
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
