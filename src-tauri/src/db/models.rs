use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct ResourceRow {
    pub url_sha256: String,
    pub url: String,
    pub title: Option<String>,
    pub status: String,
    pub content_type: Option<String>,
    pub wiki_path: Option<String>,
    pub fetched_at: Option<String>,
}

fn open(ire_dir: &Path) -> Result<Connection> {
    let db_path = ire_dir.join("local.db");
    Connection::open(&db_path).with_context(|| format!("open {}", db_path.display()))
}

pub fn insert_resource(ire_dir: &Path, sha256: &str, url: &str, content_type: &str) -> Result<()> {
    let conn = open(ire_dir)?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO resources (url_sha256, url, status, content_type, fetched_at) VALUES (?1, ?2, 'pending_summary', ?3, ?4)",
        params![sha256, url, content_type, now],
    )?;
    Ok(())
}

pub fn update_resource_status(ire_dir: &Path, sha256: &str, status: &str) -> Result<()> {
    let conn = open(ire_dir)?;
    conn.execute(
        "UPDATE resources SET status = ?1 WHERE url_sha256 = ?2",
        params![status, sha256],
    )?;
    Ok(())
}

pub fn get_resource_url(ire_dir: &Path, sha256: &str) -> Result<Option<String>> {
    let conn = open(ire_dir)?;
    let mut stmt = conn.prepare("SELECT url FROM resources WHERE url_sha256 = ?1")?;
    let mut rows = stmt.query(params![sha256])?;
    Ok(rows.next()?.map(|r| r.get(0)).transpose()?)
}

/// Mark a resource as fully indexed with its wiki path and extracted title.
pub fn update_resource_indexed(
    ire_dir: &Path,
    sha256: &str,
    wiki_path: &str,
    title: &str,
) -> Result<()> {
    let conn = open(ire_dir)?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "UPDATE resources SET status = 'summarized', wiki_path = ?1, title = ?2, summarized_at = ?3 WHERE url_sha256 = ?4",
        params![wiki_path, title, now, sha256],
    )?;
    Ok(())
}

/// Returns only confirmed (summarized) resources — the ones visible to the user.
pub fn list_resources(ire_dir: &Path) -> Result<Vec<ResourceRow>> {
    let conn = open(ire_dir)?;
    let mut stmt = conn.prepare(
        "SELECT url_sha256, url, title, status, content_type, wiki_path, fetched_at \
         FROM resources WHERE status = 'summarized' ORDER BY summarized_at DESC LIMIT 50",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ResourceRow {
            url_sha256: row.get(0)?,
            url: row.get(1)?,
            title: row.get(2)?,
            status: row.get(3)?,
            content_type: row.get(4)?,
            wiki_path: row.get(5)?,
            fetched_at: row.get(6)?,
        })
    })?;
    rows.map(|r| r.context("db row")).collect()
}
