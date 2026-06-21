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
    let mut list_key: Option<String> = None;
    let mut list_items: Vec<String> = Vec::new();

    for line in fm_text.lines() {
        if let Some(key) = list_key.clone() {
            if let Some(item) = line.trim_start().strip_prefix("- ") {
                list_items.push(item.trim().trim_matches('"').to_string());
                continue;
            }

            if list_items.is_empty() {
                map.insert(key, String::new());
            } else if let Ok(json) = serde_json::to_string(&list_items) {
                map.insert(key, json);
            }
            list_key = None;
            list_items.clear();
        }

        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let value = v.trim();
            if value.is_empty() {
                list_key = Some(key);
            } else {
                map.insert(key, value.to_string());
            }
        }
    }

    if let Some(key) = list_key {
        if list_items.is_empty() {
            map.insert(key, String::new());
        } else if let Ok(json) = serde_json::to_string(&list_items) {
            map.insert(key, json);
        }
    }

    (Some(map), body)
}
