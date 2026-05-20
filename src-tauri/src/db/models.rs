use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

// ── Experiments ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct ExperimentRow {
    pub uuid: String,
    pub name: String,
    pub command: String,
    pub status: String,
    pub exit_code: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub pid: Option<i64>,
    pub tab_id: String,
}

#[allow(clippy::too_many_arguments)]
pub fn insert_experiment(
    ire_dir: &Path,
    uuid: &str,
    name: &str,
    command: &str,
    working_dir: &str,
    wake_prompt: &str,
    session_id: &str,
    tab_id: &str,
) -> Result<()> {
    let conn = open(ire_dir)?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO experiments (uuid, name, command, working_dir, status, started_at, wake_prompt, session_id, tab_id) \
         VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7, ?8)",
        params![uuid, name, command, working_dir, now, wake_prompt, session_id, tab_id],
    )?;
    Ok(())
}

pub fn update_experiment_pid(ire_dir: &Path, uuid: &str, pid: u32) -> Result<()> {
    let conn = open(ire_dir)?;
    conn.execute(
        "UPDATE experiments SET pid = ?1 WHERE uuid = ?2",
        params![pid, uuid],
    )?;
    Ok(())
}

pub fn update_experiment_completed(
    ire_dir: &Path,
    uuid: &str,
    status: &str,
    exit_code: Option<i32>,
) -> Result<()> {
    let conn = open(ire_dir)?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "UPDATE experiments SET status = ?1, exit_code = ?2, ended_at = ?3 WHERE uuid = ?4",
        params![status, exit_code, now, uuid],
    )?;
    Ok(())
}

pub fn get_experiment(ire_dir: &Path, uuid: &str) -> Result<Option<ExperimentRow>> {
    let conn = open(ire_dir)?;
    let mut stmt = conn.prepare(
        "SELECT uuid, name, command, status, exit_code, started_at, ended_at, pid, tab_id \
         FROM experiments WHERE uuid = ?1",
    )?;
    let mut rows = stmt.query(params![uuid])?;
    rows.next()?
        .map(|r| {
            Ok::<ExperimentRow, rusqlite::Error>(ExperimentRow {
                uuid: r.get(0)?,
                name: r.get(1)?,
                command: r.get(2)?,
                status: r.get(3)?,
                exit_code: r.get(4)?,
                started_at: r.get(5)?,
                ended_at: r.get(6)?,
                pid: r.get(7)?,
                tab_id: r.get(8)?,
            })
        })
        .transpose()
        .context("get_experiment")
}

pub fn delete_experiment(ire_dir: &Path, uuid: &str) -> Result<()> {
    let conn = open(ire_dir)?;
    conn.execute("DELETE FROM experiments WHERE uuid = ?1", params![uuid])?;
    Ok(())
}

pub fn rename_experiment(ire_dir: &Path, uuid: &str, name: &str) -> Result<()> {
    let conn = open(ire_dir)?;
    conn.execute(
        "UPDATE experiments SET name = ?1 WHERE uuid = ?2",
        params![name, uuid],
    )?;
    Ok(())
}

pub fn list_experiments(ire_dir: &Path, limit: usize) -> Result<Vec<ExperimentRow>> {
    let conn = open(ire_dir)?;
    let mut stmt = conn.prepare(
        "SELECT uuid, name, command, status, exit_code, started_at, ended_at, pid, tab_id \
         FROM experiments ORDER BY started_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |r| {
        Ok(ExperimentRow {
            uuid: r.get(0)?,
            name: r.get(1)?,
            command: r.get(2)?,
            status: r.get(3)?,
            exit_code: r.get(4)?,
            started_at: r.get(5)?,
            ended_at: r.get(6)?,
            pid: r.get(7)?,
            tab_id: r.get(8)?,
        })
    })?;
    rows.map(|r| r.context("experiment row")).collect()
}

// ── Resources ─────────────────────────────────────────────────────────────────

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
