use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

fn now_ms() -> Result<i64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .map_err(|error| format!("无法读取系统时间：{error}"))
}

fn db_error(error: rusqlite::Error) -> String {
    format!("无法读写运行事件：{error}")
}

pub(crate) fn append(
    conn: &Connection,
    run_id: &str,
    task_id: &str,
    kind: &str,
    payload: Value,
) -> Result<i64, String> {
    let seq = conn
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM run_events WHERE run_id = ?1",
            [run_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(db_error)?;
    conn.execute(
        "INSERT INTO run_events (run_id, seq, kind, task_id, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![run_id, seq, kind, task_id, payload.to_string(), now_ms()?],
    )
    .map_err(db_error)?;
    Ok(seq)
}

pub(crate) fn recent_payloads(
    conn: &Connection,
    camp_id: &str,
    limit: usize,
) -> Result<Vec<Value>, String> {
    let mut statement = conn
        .prepare("SELECT payload_json FROM run_events ORDER BY rowid DESC LIMIT 2000")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    let mut payloads = Vec::new();
    for row in rows {
        let row = row.map_err(db_error)?;
        let payload: Value = serde_json::from_str(&row).map_err(|error| error.to_string())?;
        if payload.get("camp_id").and_then(Value::as_str) == Some(camp_id)
            && payload.get("role_id").is_some()
        {
            payloads.push(payload);
            if payloads.len() >= limit {
                break;
            }
        }
    }
    payloads.reverse();
    Ok(payloads)
}

#[tauri::command]
pub fn run_events(
    connection: State<'_, Mutex<Connection>>,
    camp_id: String,
) -> Result<Vec<Value>, String> {
    let connection = connection.lock().map_err(|error| error.to_string())?;
    recent_payloads(&connection, &camp_id, 500)
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PendingQuestion {
    pub(crate) id: String,
    pub(crate) camp_id: String,
    pub(crate) run_id: String,
    pub(crate) task_id: String,
    pub(crate) role_id: String,
    pub(crate) loadout_index: usize,
    pub(crate) request: String,
    pub(crate) questions: Vec<String>,
    pub(crate) answer: Option<String>,
    pub(crate) created_at: i64,
}

pub(crate) struct PendingQuestionInput<'a> {
    pub(crate) camp_id: &'a str,
    pub(crate) run_id: &'a str,
    pub(crate) task_id: &'a str,
    pub(crate) role_id: &'a str,
    pub(crate) loadout_index: usize,
    pub(crate) request: &'a str,
    pub(crate) questions: &'a [String],
}

pub(crate) fn create_pending(
    conn: &Connection,
    input: PendingQuestionInput<'_>,
) -> Result<String, String> {
    let id = format!("pending-{}", input.run_id);
    conn.execute(
        "INSERT INTO pending_questions
            (id, camp_id, run_id, task_id, role_id, loadout_index, request, questions_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            input.camp_id,
            input.run_id,
            input.task_id,
            input.role_id,
            input.loadout_index as i64,
            input.request,
            serde_json::to_string(input.questions).map_err(|error| error.to_string())?,
            now_ms()?
        ],
    )
    .map_err(db_error)?;
    Ok(id)
}

fn read_pending(row: &rusqlite::Row<'_>) -> rusqlite::Result<PendingQuestion> {
    let questions: String = row.get(7)?;
    Ok(PendingQuestion {
        id: row.get(0)?,
        camp_id: row.get(1)?,
        run_id: row.get(2)?,
        task_id: row.get(3)?,
        role_id: row.get(4)?,
        loadout_index: row.get::<_, i64>(5)? as usize,
        request: row.get(6)?,
        questions: serde_json::from_str(&questions).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        answer: row.get(8)?,
        created_at: row.get(10)?,
    })
}

const PENDING_COLUMNS: &str =
    "id, camp_id, run_id, task_id, role_id, loadout_index, request, questions_json, answer, answer_key, created_at, answered_at";

pub(crate) fn open_pending(conn: &Connection, camp_id: &str) -> Result<Vec<PendingQuestion>, String> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT {PENDING_COLUMNS} FROM pending_questions WHERE camp_id = ?1 AND answered_at IS NULL ORDER BY created_at, rowid"
        ))
        .map_err(db_error)?;
    let rows = statement
        .query_map([camp_id], read_pending)
        .map_err(db_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_error)
}

#[tauri::command]
pub fn pending_questions(
    connection: State<'_, Mutex<Connection>>,
    camp_id: String,
) -> Result<Vec<PendingQuestion>, String> {
    let connection = connection.lock().map_err(|error| error.to_string())?;
    open_pending(&connection, &camp_id)
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PendingAnswerRequest {
    pub(crate) id: String,
    pub(crate) key: String,
}

pub(crate) struct PendingAnswer {
    pub(crate) role_id: String,
    pub(crate) loadout_index: usize,
    pub(crate) request: String,
    pub(crate) questions: Vec<String>,
}

pub(crate) fn claim(
    conn: &Connection,
    camp_id: &str,
    id: &str,
    answer_key: &str,
    answer: Option<&str>,
) -> Result<Option<PendingAnswer>, String> {
    if answer_key.trim().is_empty() {
        return Err("缺少澄清答复键".into());
    }
    let changed = conn
        .execute(
            "UPDATE pending_questions SET answer = ?3, answer_key = ?4, answered_at = ?5
             WHERE id = ?1 AND camp_id = ?2 AND answered_at IS NULL",
            params![id, camp_id, answer, answer_key, now_ms()?],
        )
        .map_err(db_error)?;
    if changed == 1 {
        let row = conn
            .query_row(
                &format!("SELECT {PENDING_COLUMNS} FROM pending_questions WHERE id = ?1"),
                [id],
                read_pending,
            )
            .map_err(db_error)?;
        return Ok(Some(PendingAnswer {
            role_id: row.role_id,
            loadout_index: row.loadout_index,
            request: row.request,
            questions: row.questions,
        }));
    }
    let stored_key: Option<String> = conn
        .query_row(
            "SELECT answer_key FROM pending_questions WHERE id = ?1 AND camp_id = ?2",
            params![id, camp_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    match stored_key {
        None => Err("找不到待答问题".into()),
        Some(key) if key == answer_key => Ok(None),
        Some(_) => Err("该澄清已经答复".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_events_increment_per_run_and_pending_answers_are_idempotent() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(include_str!("../migrations/001_initial.sql"))
            .unwrap();
        connection
            .execute_batch(include_str!("../migrations/002_role_task_types.sql"))
            .unwrap();
        connection
            .execute_batch(include_str!("../migrations/004_run_events.sql"))
            .unwrap();
        assert_eq!(
            append(&connection, "run-1", "run-1", "started", serde_json::json!({"camp_id":"camp","role_id":"captain","type":"started"})).unwrap(),
            1
        );
        assert_eq!(
            append(&connection, "run-1", "run-1", "waiting", serde_json::json!({"camp_id":"camp","role_id":"captain","type":"waiting"})).unwrap(),
            2
        );
        let id = create_pending(
            &connection,
            PendingQuestionInput {
                camp_id: "camp",
                run_id: "run-1",
                task_id: "run-1",
                role_id: "captain",
                loadout_index: 0,
                request: "Fix login",
                questions: &["Which browser?".into()],
            },
        )
        .unwrap();
        assert_eq!(open_pending(&connection, "camp").unwrap().len(), 1);
        let answer = claim(&connection, "camp", &id, "key-1", Some("Chrome")).unwrap();
        assert!(answer.is_some());
        assert!(open_pending(&connection, "camp").unwrap().is_empty());
        assert!(claim(&connection, "camp", &id, "key-1", Some("Chrome")).unwrap().is_none());
        assert!(claim(&connection, "camp", &id, "key-2", Some("Firefox")).is_err());
    }
}
