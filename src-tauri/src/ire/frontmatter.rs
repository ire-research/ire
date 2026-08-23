//! Reading and writing the YAML frontmatter block of a `.ire/` markdown file.
//!
//! The parsing and the data model come from the `okf` crate, so what IRE
//! writes is an Open Knowledge Format concept and nested families like
//! `sources` and `generated` are representable from the start. What does *not*
//! come from the crate is the write path: `okf::Document::serialize` rebuilds
//! the body (`lines().join`, forced blank line, trailing-newline
//! normalization), and a status transition must leave a body an agent or a
//! person wrote exactly as they left it. So the block is located by byte
//! offset here and spliced in place, and the body is never reserialized.

use okf::yaml::Value;
use okf::Frontmatter;

const DELIM: &str = "---";

/// Split a document into its frontmatter and the body below it. The body is a
/// slice of the input, which is what lets [`replace`] put it back untouched.
/// Content with no parseable block yields `(None, content)`.
pub fn parse(content: &str) -> (Option<Frontmatter>, &str) {
    let Some((block, body)) = split(content) else {
        return (None, content);
    };
    match Value::parse(&block) {
        Ok(Value::Mapping(map)) => (Some(Frontmatter::from_mapping(map)), body),
        // A malformed or non-mapping block is not frontmatter: report none and
        // hand back the whole document, so nothing downstream half-reads it.
        _ => (None, content),
    }
}

/// Render a `---`-delimited block, key order as given.
pub fn render(frontmatter: &Frontmatter) -> String {
    let yaml = Value::Mapping(frontmatter.as_mapping().clone()).to_yaml_string();
    format!("{DELIM}\n{}{DELIM}\n", yaml)
}

/// Replace the frontmatter block, leaving the body byte-for-byte. Content with
/// no block gains one, and its body still starts exactly where it did.
pub fn replace(content: &str, frontmatter: &Frontmatter) -> String {
    // Deliberately `parse`, not `split`: `split` accepts any `---`-delimited
    // region, including a body that opens with a horizontal rule or a YAML
    // sequence. Splicing over one of those would delete real content, so the
    // two must agree on what counts as a block.
    let mut out = render(frontmatter);
    out.push_str(parse(content).1);
    out
}

/// One scalar field as a string. Non-scalars (`sources`, `generated`) have no
/// single-string reading and yield `None`; reach for the typed `okf`
/// accessors, or [`json`], for those.
pub fn field(frontmatter: &Frontmatter, key: &str) -> Option<String> {
    match frontmatter.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Int(i) => Some(i.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null | Value::Sequence(_) | Value::Mapping(_) => None,
    }
}

/// Every string in a sequence-valued field, or the single scalar if the
/// producer wrote one instead of a list.
pub fn string_list(frontmatter: &Frontmatter, key: &str) -> Vec<String> {
    match frontmatter.get(key) {
        Some(Value::Sequence(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => field(frontmatter, key)
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

/// The whole block as JSON, for crossing the IPC boundary without flattening
/// nested families into strings.
pub fn json(frontmatter: &Frontmatter) -> serde_json::Value {
    to_json(&Value::Mapping(frontmatter.as_mapping().clone()))
}

fn to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => (*b).into(),
        Value::Int(i) => (*i).into(),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        Value::String(s) => s.clone().into(),
        Value::Sequence(items) => serde_json::Value::Array(items.iter().map(to_json).collect()),
        Value::Mapping(map) => serde_json::Value::Object(
            map.iter()
                .filter_map(|(k, v)| Some((k.as_str()?.to_string(), to_json(v))))
                .collect(),
        ),
    }
}

/// Locate the block: `(its YAML text, the body slice after it)`. The body is
/// always a slice of the input, so a rewrite can put it back untouched. `\r` is
/// stripped from the returned YAML only — a CRLF file still parses, and its
/// body is handed back exactly as it was.
fn split(content: &str) -> Option<(String, &str)> {
    let after_open = {
        let rest = content.strip_prefix(DELIM)?;
        rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n'))?
    };
    let open_len = content.len() - after_open.len();

    let mut offset = open_len;
    for line in after_open.split_inclusive('\n') {
        if line.trim_end_matches('\n').trim_end_matches('\r') == DELIM {
            let block = content[open_len..offset].replace('\r', "");
            return Some((block, &content[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "---\ntype: Experiment\ntitle: LR ablation\nrun_status: running\n---\n\n# LR ablation\n\nbody text\n";

    #[test]
    fn crlf_files_parse_and_keep_their_body() {
        // What a `core.autocrlf=true` checkout hands us on Windows.
        let doc = "---\r\ntype: Experiment\r\nrun_status: running\r\n---\r\n\r\n# T\r\nnotes\r\n";
        let (fm, body) = parse(doc);
        let fm = fm.unwrap();
        assert_eq!(field(&fm, "run_status").as_deref(), Some("running"));
        assert_eq!(body, "\r\n# T\r\nnotes\r\n", "body must survive byte-for-byte");

        let mut next = fm;
        next.set("run_status", Value::String("completed".into()));
        assert_eq!(parse(&replace(doc, &next)).1, body);
    }

    #[test]
    fn replace_never_eats_a_body_that_only_looks_like_frontmatter() {
        // `---` opening a YAML sequence, and a leading horizontal rule: `parse`
        // rejects both, so `replace` must prepend rather than splice over them.
        for doc in [
            "---\n- a\n- b\n---\n\nbody\n",
            "---\n\nsome text\n\n---\n\nmore\n",
        ] {
            let mut fm = Frontmatter::new();
            fm.set("type", Value::String("Experiment".into()));
            let out = replace(doc, &fm);
            assert!(out.ends_with(doc), "content was destroyed: {out:?}");
        }
    }

    #[test]
    fn a_block_closed_at_end_of_file_has_an_empty_body() {
        let (fm, body) = parse("---\ntype: Experiment\n---");
        assert!(fm.is_some());
        assert_eq!(body, "");
    }

    #[test]
    fn parses_scalars_and_the_body_slice() {
        let (fm, body) = parse(DOC);
        let fm = fm.unwrap();
        assert_eq!(field(&fm, "type").as_deref(), Some("Experiment"));
        assert_eq!(field(&fm, "title").as_deref(), Some("LR ablation"));
        assert_eq!(body, "\n# LR ablation\n\nbody text\n");
    }

    #[test]
    fn replace_leaves_the_body_byte_for_byte() {
        let (fm, body) = parse(DOC);
        let mut fm = fm.unwrap();
        fm.set("run_status", Value::String("completed".into()));
        let out = replace(DOC, &fm);

        assert_eq!(parse(&out).1, body);
        assert!(out.contains("run_status: completed"));
        assert!(!out.contains("run_status: running"));
    }

    #[test]
    fn key_order_survives_a_rewrite() {
        let (fm, _) = parse(DOC);
        let mut fm = fm.unwrap();
        fm.set("run_status", Value::String("failed".into()));
        let out = replace(DOC, &fm);
        let keys: Vec<&str> = out
            .lines()
            .skip(1)
            .take_while(|l| *l != "---")
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(keys, ["type", "title", "run_status"]);
    }

    #[test]
    fn nested_families_round_trip() {
        let doc = "---\ntype: Claim\nsources:\n  - id: a\n    resource: https://example.com/a\ngenerated:\n  by: ire/claude\n  at: 2026-08-21T10:00:00Z\n---\n\nbody\n";
        let (fm, _) = parse(doc);
        let fm = fm.unwrap();
        assert_eq!(fm.sources().len(), 1);
        assert_eq!(fm.generated().unwrap().by.unwrap().as_str(), "ire/claude");

        // The families survive a rewrite that never mentions them.
        let mut next = fm.clone();
        next.set("title", Value::String("Scaling holds".into()));
        let out = replace(doc, &next);
        let (again, _) = parse(&out);
        assert_eq!(again.unwrap().sources().len(), 1);
    }

    #[test]
    fn content_without_frontmatter_is_left_whole() {
        let (fm, body) = parse("# just a heading\n");
        assert!(fm.is_none());
        assert_eq!(body, "# just a heading\n");
    }

    #[test]
    fn a_malformed_block_is_not_half_read() {
        let doc = "---\ntype: [unclosed\n---\n\nbody\n";
        let (fm, body) = parse(doc);
        assert!(fm.is_none());
        assert_eq!(body, doc);
    }

    #[test]
    fn string_list_reads_a_list_or_a_lone_scalar() {
        let (fm, _) = parse("---\ntype: Reference\nsources:\n  - https://a\n  - https://b\n---\n");
        assert_eq!(
            string_list(&fm.unwrap(), "sources"),
            ["https://a", "https://b"]
        );

        let (fm, _) = parse("---\ntype: Reference\nsources: https://only\n---\n");
        assert_eq!(string_list(&fm.unwrap(), "sources"), ["https://only"]);
    }
}
