use crate::services::hierarchy::{new_id, now, DbState};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobRecord {
    pub id: String,
    pub job_type: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub status: String,
    pub progress_current: i32,
    pub progress_total: i32,
    pub message: Option<String>,
    pub error: Option<String>,
    pub cancel_requested: bool,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub updated_at: String,
}

pub fn create_job(
    conn: &Connection,
    job_type: &str,
    entity_type: Option<&str>,
    entity_id: Option<&str>,
    progress_total: i32,
    message: Option<&str>,
) -> Result<String, String> {
    let id = new_id("job");
    let timestamp = now();
    conn.execute(
        "INSERT INTO jobs (id, job_type, entity_type, entity_id, status, progress_current, progress_total, message, error, cancel_requested, created_at, started_at, finished_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'queued', 0, ?5, ?6, NULL, 0, ?7, NULL, NULL, ?7)",
        params![id, job_type, entity_type, entity_id, progress_total.max(0), message, timestamp],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn mark_job_running(
    conn: &Connection,
    job_id: &str,
    message: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE jobs SET status = 'running', started_at = COALESCE(started_at, ?1), updated_at = ?1, message = COALESCE(?2, message) WHERE id = ?3",
        params![now(), message, job_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_job_progress(
    conn: &Connection,
    job_id: &str,
    current: i32,
    total: i32,
    message: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE jobs SET progress_current = ?1, progress_total = ?2, message = COALESCE(?3, message), updated_at = ?4 WHERE id = ?5",
        params![current.max(0), total.max(0), message, now(), job_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn complete_job(conn: &Connection, job_id: &str, message: Option<&str>) -> Result<(), String> {
    conn.execute(
        "UPDATE jobs SET status = 'completed', message = COALESCE(?1, message), error = NULL, finished_at = ?2, updated_at = ?2 WHERE id = ?3",
        params![message, now(), job_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn fail_job(conn: &Connection, job_id: &str, error: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE jobs SET status = 'failed', error = ?1, finished_at = ?2, updated_at = ?2 WHERE id = ?3",
        params![error, now(), job_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn is_cancel_requested(conn: &Connection, job_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT cancel_requested FROM jobs WHERE id = ?1",
        params![job_id],
        |row| row.get::<_, bool>(0),
    )
    .map_err(|e| e.to_string())
}

fn job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobRecord> {
    Ok(JobRecord {
        id: row.get(0)?,
        job_type: row.get(1)?,
        entity_type: row.get(2)?,
        entity_id: row.get(3)?,
        status: row.get(4)?,
        progress_current: row.get(5)?,
        progress_total: row.get(6)?,
        message: row.get(7)?,
        error: row.get(8)?,
        cancel_requested: row.get(9)?,
        created_at: row.get(10)?,
        started_at: row.get(11)?,
        finished_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

#[tauri::command]
pub async fn get_job(state: DbState<'_>, job_id: String) -> Result<JobRecord, String> {
    state.with_conn(|conn| {
        conn.query_row(
            "SELECT id, job_type, entity_type, entity_id, status, progress_current, progress_total, message, error, cancel_requested, created_at, started_at, finished_at, updated_at FROM jobs WHERE id = ?1",
            params![job_id],
            job_from_row,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub async fn get_recent_jobs(
    state: DbState<'_>,
    limit: Option<i32>,
) -> Result<Vec<JobRecord>, String> {
    state.with_conn(|conn| {
        let limit = limit.unwrap_or(25).clamp(1, 100);
        let mut stmt = conn
            .prepare("SELECT id, job_type, entity_type, entity_id, status, progress_current, progress_total, message, error, cancel_requested, created_at, started_at, finished_at, updated_at FROM jobs ORDER BY created_at DESC LIMIT ?1")
            .map_err(|e| e.to_string())?;
        let jobs = stmt
            .query_map(params![limit], job_from_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(jobs)
    })
}

#[tauri::command]
pub async fn cancel_job(state: DbState<'_>, job_id: String) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE jobs SET cancel_requested = 1, status = CASE WHEN status IN ('queued', 'running') THEN 'cancelling' ELSE status END, updated_at = ?1 WHERE id = ?2",
            params![now(), job_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_lifecycle_persists_progress_and_error() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/schema.sql"))
            .unwrap();
        let job_id = create_job(&conn, "grading", Some("assessment"), Some("a1"), 3, None).unwrap();
        mark_job_running(&conn, &job_id, Some("started")).unwrap();
        update_job_progress(&conn, &job_id, 1, 3, Some("one done")).unwrap();
        fail_job(&conn, &job_id, "provider unavailable").unwrap();

        let row = conn
            .query_row(
                "SELECT id, job_type, entity_type, entity_id, status, progress_current, progress_total, message, error, cancel_requested, created_at, started_at, finished_at, updated_at FROM jobs WHERE id = ?1",
                params![job_id],
                job_from_row,
            )
            .unwrap();
        assert_eq!(row.status, "failed");
        assert_eq!(row.progress_current, 1);
        assert_eq!(row.error.as_deref(), Some("provider unavailable"));
    }
}
