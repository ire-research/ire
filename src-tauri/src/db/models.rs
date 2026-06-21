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
    home_data_dir: &Path,
    uuid: &str,
    name: &str,
    command: &str,
    working_dir: &str,
    wake_prompt: &str,
    session_id: &str,
    tab_id: &str,
) -> Result<()> {
    let conn = open(home_data_dir)?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO experiments (uuid, name, command, working_dir, status, started_at, wake_prompt, session_id, tab_id) \
         VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7, ?8)",
        params![uuid, name, command, working_dir, now, wake_prompt, session_id, tab_id],
    )?;
    Ok(())
}

pub fn update_experiment_pid(home_data_dir: &Path, uuid: &str, pid: u32) -> Result<()> {
    let conn = open(home_data_dir)?;
    conn.execute(
        "UPDATE experiments SET pid = ?1 WHERE uuid = ?2",
        params![pid, uuid],
    )?;
    Ok(())
}

pub fn update_experiment_completed(
    home_data_dir: &Path,
    uuid: &str,
    status: &str,
    exit_code: Option<i32>,
) -> Result<()> {
    let conn = open(home_data_dir)?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "UPDATE experiments SET status = ?1, exit_code = ?2, ended_at = ?3 WHERE uuid = ?4",
        params![status, exit_code, now, uuid],
    )?;
    Ok(())
}

pub fn get_experiment(home_data_dir: &Path, uuid: &str) -> Result<Option<ExperimentRow>> {
    let conn = open(home_data_dir)?;
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

pub fn delete_experiment(home_data_dir: &Path, uuid: &str) -> Result<()> {
    let conn = open(home_data_dir)?;
    conn.execute("DELETE FROM experiments WHERE uuid = ?1", params![uuid])?;
    Ok(())
}

pub fn rename_experiment(home_data_dir: &Path, uuid: &str, name: &str) -> Result<()> {
    let conn = open(home_data_dir)?;
    conn.execute(
        "UPDATE experiments SET name = ?1 WHERE uuid = ?2",
        params![name, uuid],
    )?;
    Ok(())
}

pub fn list_experiments(home_data_dir: &Path, limit: usize) -> Result<Vec<ExperimentRow>> {
    let conn = open(home_data_dir)?;
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

fn open(home_data_dir: &Path) -> Result<Connection> {
    let db_path = home_data_dir.join("local.db");
    Connection::open(&db_path).with_context(|| format!("open {}", db_path.display()))
}

// ── Chat Sessions ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct ChatSessionRow {
    pub session_uuid: String,
    pub tab_label: String,
    pub provider: String,
    pub model: String,
    pub started_at: String,
    pub ended_at: String,
    pub message_count: i64,
    pub first_user_msg: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn insert_chat_session(
    home_data_dir: &Path,
    session_uuid: &str,
    tab_label: &str,
    provider: &str,
    model: &str,
    started_at: &str,
    ended_at: &str,
    message_count: i64,
    first_user_msg: Option<&str>,
    messages_json: &str,
) -> Result<()> {
    let conn = open(home_data_dir)?;
    // Upsert: insert on first save, update mutable fields on subsequent saves.
    // started_at is intentionally excluded from the UPDATE so it stays as the
    // original session start time even as the session grows.
    conn.execute(
        "INSERT INTO chat_sessions \
         (session_uuid, tab_label, provider, model, started_at, ended_at, message_count, first_user_msg, messages_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT(session_uuid) DO UPDATE SET \
             tab_label      = excluded.tab_label, \
             provider       = excluded.provider, \
             model          = excluded.model, \
             ended_at       = excluded.ended_at, \
             message_count  = excluded.message_count, \
             first_user_msg = excluded.first_user_msg, \
             messages_json  = excluded.messages_json",
        params![session_uuid, tab_label, provider, model, started_at, ended_at, message_count, first_user_msg, messages_json],
    )?;
    Ok(())
}

/// Persist the agent resume id for a session, keyed by `session_uuid`. Creates a
/// minimal row if the session has not been saved yet (the first `Init` arrives
/// before any messages are written). The resume column is chosen by provider.
#[allow(clippy::too_many_arguments)]
pub fn upsert_chat_resume_id(
    home_data_dir: &Path,
    session_uuid: &str,
    tab_label: &str,
    provider: &str,
    model: &str,
    started_at: &str,
    resume_id: &str,
) -> Result<()> {
    let col = resume_column(provider);
    let conn = open(home_data_dir)?;
    let now = chrono::Local::now().to_rfc3339();
    let sql = format!(
        "INSERT INTO chat_sessions \
         (session_uuid, tab_label, provider, model, started_at, ended_at, message_count, first_user_msg, messages_json, {col}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, '[]', ?7) \
         ON CONFLICT(session_uuid) DO UPDATE SET provider = excluded.provider, {col} = excluded.{col}"
    );
    conn.execute(
        &sql,
        params![session_uuid, tab_label, provider, model, started_at, now, resume_id],
    )?;
    Ok(())
}

/// Update the resume id for an existing session. Used by the experiment wake-up
/// flow, where the row is guaranteed to exist (the conversation that launched the
/// experiment was already saved).
pub fn update_chat_resume_id(
    home_data_dir: &Path,
    session_uuid: &str,
    provider: &str,
    resume_id: &str,
) -> Result<()> {
    let col = resume_column(provider);
    let conn = open(home_data_dir)?;
    let sql = format!("UPDATE chat_sessions SET {col} = ?1 WHERE session_uuid = ?2");
    conn.execute(&sql, params![resume_id, session_uuid])?;
    Ok(())
}

/// Read the persisted resume id for a session and provider, if any.
pub fn get_chat_resume_id(
    home_data_dir: &Path,
    session_uuid: &str,
    provider: &str,
) -> Result<Option<String>> {
    let col = resume_column(provider);
    let conn = open(home_data_dir)?;
    let sql = format!("SELECT {col} FROM chat_sessions WHERE session_uuid = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![session_uuid])?;
    let val: Option<Option<String>> = rows
        .next()?
        .map(|r| r.get::<_, Option<String>>(0))
        .transpose()?;
    Ok(val.flatten())
}

/// Resume-id column for a provider. Codex calls it a thread; Claude a session.
fn resume_column(provider: &str) -> &'static str {
    if provider == "codex" {
        "codex_thread_id"
    } else {
        "claude_session_id"
    }
}

pub fn list_chat_sessions(home_data_dir: &Path, limit: usize) -> Result<Vec<ChatSessionRow>> {
    let conn = open(home_data_dir)?;
    let mut stmt = conn.prepare(
        "SELECT session_uuid, tab_label, provider, model, started_at, ended_at, message_count, first_user_msg \
         FROM chat_sessions WHERE message_count > 0 ORDER BY ended_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |r| {
        Ok(ChatSessionRow {
            session_uuid: r.get(0)?,
            tab_label: r.get(1)?,
            provider: r.get(2)?,
            model: r.get(3)?,
            started_at: r.get(4)?,
            ended_at: r.get(5)?,
            message_count: r.get(6)?,
            first_user_msg: r.get(7)?,
        })
    })?;
    rows.map(|r| r.context("chat session row")).collect()
}

pub fn get_chat_session_messages(home_data_dir: &Path, session_uuid: &str) -> Result<Option<String>> {
    let conn = open(home_data_dir)?;
    let mut stmt =
        conn.prepare("SELECT messages_json FROM chat_sessions WHERE session_uuid = ?1")?;
    let mut rows = stmt.query(params![session_uuid])?;
    rows.next()?
        .map(|r| r.get::<_, String>(0))
        .transpose()
        .context("get_chat_session_messages")
}

pub fn delete_chat_session(home_data_dir: &Path, session_uuid: &str) -> Result<()> {
    let conn = open(home_data_dir)?;
    conn.execute(
        "DELETE FROM chat_sessions WHERE session_uuid = ?1",
        params![session_uuid],
    )?;
    Ok(())
}

