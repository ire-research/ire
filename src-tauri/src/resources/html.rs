use scraper::ElementRef;

pub fn extract_text_from_html(html: &str) -> String {
    let doc = scraper::Html::parse_document(html);
    let mut parts = Vec::new();
    collect_text(doc.root_element(), &mut parts);
    // Trim whitespace and join
    let text = parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    // Hard cap at 300 KB to avoid enormous context injections
    if text.len() > 300_000 {
        text[..300_000].to_string()
    } else {
        text
    }
}

fn collect_text(element: ElementRef, out: &mut Vec<String>) {
    let tag = element.value().name();
    if matches!(
        tag,
        "script" | "style" | "nav" | "header" | "footer" | "aside" | "noscript" | "iframe"
    ) {
        return;
    }

    for child in element.children() {
        match child.value() {
            scraper::node::Node::Text(text) => {
                let t = text.trim();
                if !t.is_empty() {
                    out.push(t.to_string());
                }
            }
            scraper::node::Node::Element(_) => {
                if let Some(child_elem) = ElementRef::wrap(child) {
                    collect_text(child_elem, out);
                }
            }
            _ => {}
        }
    }
}
