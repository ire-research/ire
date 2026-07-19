use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::Result;

use super::frontmatter;

/// Claim relation frontmatter keys that must point at existing claim ids.
const RELATION_KEYS: [&str; 3] = ["depends_on", "contradicts", "supersedes"];

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
            .map(|t| frontmatter::unquote(t))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| name.trim_end_matches(".md").to_string());
        let summary = fm
            .as_ref()
            .and_then(|m| m.get("TL;DR").or_else(|| m.get("summary")))
            .map(|s| frontmatter::unquote(s))
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

struct ClaimEntry {
    name: String,
    id: String,
    status: String,
    statement: String,
    relations: Vec<String>,
}

/// Build `claims/_index.md` (one bullet per `claims/*.md` file, excluding
/// `_index.md` and dotfiles, sorted by filename) and report dangling relation
/// references: ids named in `depends_on`/`contradicts`/`supersedes` frontmatter
/// that have no matching claim file. Returns `(index_markdown, dangling_by_id)`
/// — `dangling_by_id` only contains claims that reference at least one missing
/// id, keyed by the referencing claim's id.
pub fn build_claims(claims_dir: &Path) -> Result<(String, BTreeMap<String, Vec<String>>)> {
    let Ok(read_dir) = fs::read_dir(claims_dir) else {
        return Ok((String::new(), BTreeMap::new()));
    };

    let mut entries: Vec<ClaimEntry> = vec![];
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
            .map(|s| frontmatter::unquote(s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "proposed".to_string());
        let statement = first_paragraph(body).unwrap_or_default();
        let mut relations = vec![];
        if let Some(m) = fm.as_ref() {
            for key in RELATION_KEYS {
                if let Some(v) = m.get(key) {
                    relations.extend(frontmatter::parse_list(v));
                }
            }
        }
        let id = name.trim_end_matches(".md").to_string();
        entries.push(ClaimEntry { name, id, status, statement, relations });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let known: HashSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();

    let mut out = String::new();
    let mut dangling: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in &entries {
        if e.statement.is_empty() {
            out.push_str(&format!("- [{}](./{}) — {}\n", e.id, e.name, e.status));
        } else {
            out.push_str(&format!("- [{}](./{}) — {} — {}\n", e.id, e.name, e.status, e.statement));
        }
        let missing: Vec<String> = e
            .relations
            .iter()
            .filter(|r| !known.contains(r.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            out.push_str(&format!(
                "  ⚠ dangling reference{}: {}\n",
                if missing.len() > 1 { "s" } else { "" },
                missing.join(", ")
            ));
            dangling.insert(e.id.clone(), missing);
        }
    }
    Ok((out, dangling))
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
        let (out, dangling) = build_claims(dir.path()).unwrap();
        assert_eq!(
            out,
            "- [head-not-bottleneck](./head-not-bottleneck.md) — supported — The head is not the bottleneck.\n"
        );
        assert!(dangling.is_empty());
    }

    #[test]
    fn build_claims_defaults_status_to_proposed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("open-question.md"), "no frontmatter\n").unwrap();
        let (out, _) = build_claims(dir.path()).unwrap();
        assert!(out.starts_with("- [open-question](./open-question.md) — proposed"));
    }

    #[test]
    fn build_claims_flags_dangling_relation() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("claim-a.md"),
            "---\ntype: Claim\nid: claim-a\nstatus: proposed\ndepends_on:\n  - claim-b\n---\n\nSomething.\n",
        )
        .unwrap();
        let (out, dangling) = build_claims(dir.path()).unwrap();
        assert!(out.contains("⚠ dangling reference: claim-b"));
        assert_eq!(dangling.get("claim-a").unwrap(), &vec!["claim-b".to_string()]);
    }

    #[test]
    fn build_claims_does_not_flag_existing_relation() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("claim-a.md"),
            "---\ntype: Claim\nid: claim-a\nstatus: proposed\ndepends_on:\n  - claim-b\n---\n\nSomething.\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("claim-b.md"),
            "---\ntype: Claim\nid: claim-b\nstatus: supported\n---\n\nSomething else.\n",
        )
        .unwrap();
        let (out, dangling) = build_claims(dir.path()).unwrap();
        assert!(!out.contains('⚠'));
        assert!(dangling.is_empty());
    }
}
