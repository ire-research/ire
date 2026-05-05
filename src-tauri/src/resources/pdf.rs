use std::io::Write;

use anyhow::{Context, Result};

pub fn extract_text_from_bytes(bytes: &[u8]) -> Result<String> {
    let mut tmp = tempfile::Builder::new()
        .suffix(".pdf")
        .tempfile()
        .context("create temp file")?;
    tmp.write_all(bytes).context("write pdf bytes")?;
    tmp.flush()?;

    let text = pdf_extract::extract_text(tmp.path()).context("PDF text extraction")?;

    // Hard cap at 300 KB
    if text.len() > 300_000 {
        Ok(text[..300_000].to_string())
    } else {
        Ok(text)
    }
}
