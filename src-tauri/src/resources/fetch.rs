use std::time::Duration;

use anyhow::{Context, Result};

pub struct FetchResult {
    pub text: String,
    pub content_type: String,
}

pub fn fetch_and_extract(url: &str) -> Result<FetchResult> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("IRE/0.1 (academic research assistant)")
        .build()
        .context("build HTTP client")?;

    let response = client.get(url).send().context("HTTP request")?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!("HTTP {status}"));
    }

    let raw_ct = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html")
        .to_string();

    let content_type = raw_ct
        .split(';')
        .next()
        .unwrap_or("text/html")
        .trim()
        .to_string();

    let is_pdf = content_type.contains("application/pdf");

    let bytes = response.bytes().context("read response body")?;

    let text = if is_pdf {
        super::pdf::extract_text_from_bytes(&bytes)?
    } else {
        let html = String::from_utf8_lossy(&bytes).to_string();
        super::html::extract_text_from_html(&html)
    };

    Ok(FetchResult { text, content_type })
}
