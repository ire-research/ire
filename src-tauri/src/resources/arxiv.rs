//! arXiv e-print fetching: download the LaTeX tarball instead of the PDF.
//! Cleaner text than `pdf-extract` for math-heavy papers.

use std::io::Read;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;

/// Extract an arXiv id from common URL forms. Returns `None` if not arXiv.
///
/// Handles:
/// - `https://arxiv.org/abs/2401.12345`
/// - `https://arxiv.org/abs/2401.12345v2`
/// - `https://arxiv.org/pdf/2401.12345(.pdf)`
/// - old-style `https://arxiv.org/abs/cs.LG/0501001`
pub fn parse_arxiv_id(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    if !lower.contains("arxiv.org/") {
        return None;
    }
    // Strip protocol + host.
    let after_host = url.split("arxiv.org/").nth(1)?;
    // Drop `abs/` or `pdf/` prefix; anything else (e.g. `format/`) is unsupported.
    let rest = after_host
        .strip_prefix("abs/")
        .or_else(|| after_host.strip_prefix("pdf/"))?;
    // Strip trailing `.pdf` and any query/fragment.
    let id = rest
        .split(['?', '#']).next().unwrap_or(rest)
        .trim_end_matches(".pdf")
        .trim_end_matches('/');
    if id.is_empty() {
        return None;
    }
    Some(id.to_string())
}

/// Fetch the e-print archive for a given arXiv id and extract the LaTeX
/// source as plain text. The e-print is usually a gzipped tarball of `.tex`
/// files; sometimes a single gzipped `.tex`; rarely a raw PDF (in which case
/// the caller should fall back to PDF extraction).
pub fn fetch_latex_source(id: &str) -> Result<String> {
    let url = format!("https://arxiv.org/e-print/{id}");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("IRE/0.1 (academic research assistant)")
        .build()
        .context("build HTTP client")?;

    let response = client.get(&url).send().context("HTTP request")?;
    if !response.status().is_success() {
        return Err(anyhow!("HTTP {} fetching {}", response.status(), url));
    }
    let bytes = response.bytes().context("read response body")?;

    extract_latex_from_eprint(&bytes)
}

/// Best-effort extraction. Tries gzip + tar, then gzip-only, then raw.
fn extract_latex_from_eprint(bytes: &[u8]) -> Result<String> {
    // Path 1: gzipped tarball — the common case.
    if let Ok(mut decoder) = try_gunzip(bytes) {
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        // Try as tar first.
        if let Ok(text) = read_tar_tex(&decompressed) {
            return Ok(text);
        }
        // Fall back: single .tex.gz.
        if let Ok(text) = std::str::from_utf8(&decompressed) {
            return Ok(text.to_string());
        }
        return Err(anyhow!("e-print decompressed but not utf-8 tex"));
    }

    // Path 2: not gzipped — raw bytes might be tex or PDF.
    if bytes.starts_with(b"%PDF") {
        return Err(anyhow!("e-print is a PDF, not LaTeX source"));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.to_string());
    }
    Err(anyhow!("e-print is not in a recognised format"))
}

fn try_gunzip(bytes: &[u8]) -> Result<GzDecoder<&[u8]>> {
    if bytes.len() < 2 || bytes[0] != 0x1f || bytes[1] != 0x8b {
        return Err(anyhow!("not gzip"));
    }
    Ok(GzDecoder::new(bytes))
}

/// Walk the tar, collect all `.tex` files. Pick the entry containing
/// `\documentclass` as the main file and prepend it; concatenate the rest
/// so that `\input`/`\include` material is also visible to the summariser.
fn read_tar_tex(bytes: &[u8]) -> Result<String> {
    let mut archive = tar::Archive::new(bytes);
    let mut main: Option<String> = None;
    let mut others: Vec<String> = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else { continue };
        if !ext.eq_ignore_ascii_case("tex") {
            continue;
        }
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_err() {
            continue;
        }
        if main.is_none() && content.contains("\\documentclass") {
            main = Some(content);
        } else {
            others.push(content);
        }
    }

    match main {
        Some(m) => {
            let mut combined = m;
            for o in others {
                combined.push_str("\n\n");
                combined.push_str(&o);
            }
            Ok(combined)
        }
        None if !others.is_empty() => Ok(others.join("\n\n")),
        None => Err(anyhow!("no .tex files in archive")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_abs_id() {
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/abs/2401.12345"),
            Some("2401.12345".to_string())
        );
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/abs/2401.12345v2"),
            Some("2401.12345v2".to_string())
        );
    }

    #[test]
    fn parses_pdf_id() {
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/pdf/2401.12345.pdf"),
            Some("2401.12345".to_string())
        );
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/pdf/2401.12345v2"),
            Some("2401.12345v2".to_string())
        );
    }

    #[test]
    fn parses_old_style_id() {
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/abs/cs.LG/0501001"),
            Some("cs.LG/0501001".to_string())
        );
    }

    #[test]
    fn rejects_non_arxiv() {
        assert_eq!(parse_arxiv_id("https://example.com/abs/2401.12345"), None);
    }
}
