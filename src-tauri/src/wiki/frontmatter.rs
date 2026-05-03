use std::collections::HashMap;

/// Parse YAML frontmatter delimited by `---`. Returns (map, body_after_delimiter).
/// Returns (None, full_content) if no frontmatter is found.
pub fn parse(content: &str) -> (Option<HashMap<String, String>>, &str) {
    let Some(rest) = content.strip_prefix("---\n") else {
        return (None, content);
    };
    let (fm_text, body) = if let Some(idx) = rest.find("\n---\n") {
        (&rest[..idx], &rest[idx + 5..])
    } else if let Some(idx) = rest.find("\n---") {
        (&rest[..idx], "")
    } else {
        return (None, content);
    };

    let mut map = HashMap::new();
    for line in fm_text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    (Some(map), body)
}
