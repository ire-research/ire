use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{bail, Context, Result};

pub struct LocalFileResult {
    pub text: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub fn extract_local_file(path: &Path) -> Result<LocalFileResult> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;

    let (text, content_type) = match extension.as_str() {
        "txt" => (
            String::from_utf8(bytes.clone()).context("read .txt as UTF-8")?,
            "text/plain",
        ),
        "md" => (
            String::from_utf8(bytes.clone()).context("read .md as UTF-8")?,
            "text/markdown",
        ),
        "pdf" => (
            super::pdf::extract_text_from_bytes(&bytes)?,
            "application/pdf",
        ),
        "docx" => (
            extract_docx_text(&bytes)?,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        _ => bail!("unsupported file type: expected .txt, .md, .pdf, or .docx"),
    };

    Ok(LocalFileResult {
        text: cap_text(text),
        content_type: content_type.to_string(),
        bytes,
    })
}

fn cap_text(text: String) -> String {
    if text.len() > 300_000 {
        text[..300_000].to_string()
    } else {
        text
    }
}

fn extract_docx_text(bytes: &[u8]) -> Result<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).context("open .docx package")?;
    let mut document = archive
        .by_name("word/document.xml")
        .context("read word/document.xml from .docx")?;

    let mut xml = String::new();
    document
        .read_to_string(&mut xml)
        .context("read .docx document XML")?;

    let doc = roxmltree::Document::parse(&xml).context("parse .docx document XML")?;
    let mut paragraphs = Vec::new();

    for paragraph in doc.descendants().filter(|n| n.has_tag_name("p")) {
        let mut text = String::new();
        for node in paragraph.descendants() {
            if node.has_tag_name("t") {
                if let Some(part) = node.text() {
                    text.push_str(part);
                }
            } else if node.has_tag_name("tab") {
                text.push('\t');
            } else if node.has_tag_name("br") {
                text.push('\n');
            }
        }

        let trimmed = text.trim();
        if !trimmed.is_empty() {
            paragraphs.push(trimmed.to_string());
        }
    }

    if paragraphs.is_empty() {
        bail!("no text found in .docx");
    }

    Ok(paragraphs.join("\n"))
}
