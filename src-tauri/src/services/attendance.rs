use crate::services::hierarchy::{new_id, now, DbState};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub class_id: String,
    pub year_plan_lesson_id: Option<String>,
    pub session_date: String,
    pub title: String,
    pub description: Option<String>,
    pub topic_tags_json: String,
    pub completed: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceView {
    pub id: String,
    pub session_id: String,
    pub student_id: String,
    pub student_name: String,
    pub status: String,
    pub note: Option<String>,
    pub recorded_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CalendarClassSessionView {
    pub session: SessionView,
    pub class_name: String,
    pub level_name: String,
    pub school_name: String,
    pub subject_name: Option<String>,
}

#[tauri::command]
pub async fn create_session(
    state: DbState<'_>,
    class_id: String,
    year_plan_lesson_id: Option<String>,
    session_date: String,
    title: String,
    description: Option<String>,
    topic_tags_json: Option<String>,
) -> Result<String, String> {
    state.with_conn(|conn| {
        let id = new_id("session");
        let timestamp = now();
        conn.execute(
            "INSERT INTO sessions (id, class_id, year_plan_lesson_id, session_date, title, description, topic_tags_json, completed, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9)",
            params![
                id,
                class_id,
                year_plan_lesson_id,
                session_date,
                title,
                description,
                topic_tags_json.unwrap_or_else(|| "[]".to_string()),
                timestamp,
                timestamp
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    })
}

#[tauri::command]
pub async fn get_sessions_by_class(
    state: DbState<'_>,
    class_id: String,
) -> Result<Vec<SessionView>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, class_id, year_plan_lesson_id, session_date, title, description, topic_tags_json, completed, created_at, updated_at FROM sessions WHERE class_id = ?1 ORDER BY session_date DESC")
            .map_err(|e| e.to_string())?;
        let result = stmt.query_map(params![class_id], |row| {
            Ok(SessionView {
                id: row.get(0)?,
                class_id: row.get(1)?,
                year_plan_lesson_id: row.get(2)?,
                session_date: row.get(3)?,
                title: row.get(4)?,
                description: row.get(5)?,
                topic_tags_json: row.get(6)?,
                completed: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn get_all_calendar_sessions(
    state: DbState<'_>,
) -> Result<Vec<CalendarClassSessionView>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.class_id, s.year_plan_lesson_id, s.session_date, s.title, s.description, s.topic_tags_json, s.completed, s.created_at, s.updated_at,
                        c.name, c.subject_name, lev.name, sch.name
                 FROM sessions s
                 JOIN classes c ON s.class_id = c.id
                 JOIN levels lev ON c.level_id = lev.id
                 JOIN schools sch ON c.school_id = sch.id
                 WHERE c.archived_at IS NULL AND sch.archived_at IS NULL
                 ORDER BY s.session_date, sch.name, lev.display_order, c.name",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_map([], |row| {
                Ok(CalendarClassSessionView {
                    session: SessionView {
                        id: row.get(0)?,
                        class_id: row.get(1)?,
                        year_plan_lesson_id: row.get(2)?,
                        session_date: row.get(3)?,
                        title: row.get(4)?,
                        description: row.get(5)?,
                        topic_tags_json: row.get(6)?,
                        completed: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                    },
                    class_name: row.get(10)?,
                    subject_name: row.get(11)?,
                    level_name: row.get(12)?,
                    school_name: row.get(13)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn create_attendance_record(
    state: DbState<'_>,
    session_id: String,
    student_id: String,
    status: String,
    note: Option<String>,
) -> Result<String, String> {
    state.with_conn(|conn| {
        upsert_attendance_record(conn, &session_id, &student_id, &status, note.as_deref())
    })
}

#[tauri::command]
pub async fn get_attendance_by_session(
    state: DbState<'_>,
    session_id: String,
) -> Result<Vec<AttendanceView>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT ar.id, ar.session_id, ar.student_id, s.full_name, ar.status, ar.note, ar.recorded_at, ar.updated_at
                 FROM attendance_records ar
                 JOIN students s ON ar.student_id = s.id
                 WHERE ar.session_id = ?1
                 ORDER BY s.full_name",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt.query_map(params![session_id], |row| {
            Ok(AttendanceView {
                id: row.get(0)?,
                session_id: row.get(1)?,
                student_id: row.get(2)?,
                student_name: row.get(3)?,
                status: row.get(4)?,
                note: row.get(5)?,
                recorded_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn update_session_completion(
    state: DbState<'_>,
    session_id: String,
    completed: bool,
) -> Result<(), String> {
    state.with_conn(|conn| set_session_completion(conn, &session_id, completed))
}

fn upsert_attendance_record(
    conn: &Connection,
    session_id: &str,
    student_id: &str,
    status: &str,
    note: Option<&str>,
) -> Result<String, String> {
    let id = new_id("attendance");
    let timestamp = now();
    conn.execute(
        "INSERT INTO attendance_records (id, session_id, student_id, status, note, recorded_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(session_id, student_id) DO UPDATE SET status = excluded.status, note = excluded.note, updated_at = excluded.updated_at",
        params![id, session_id, student_id, status, note, timestamp, timestamp],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

fn set_session_completion(
    conn: &Connection,
    session_id: &str,
    completed: bool,
) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET completed = ?1, updated_at = ?2 WHERE id = ?3",
        params![completed, now(), session_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                class_id TEXT NOT NULL,
                year_plan_lesson_id TEXT,
                session_date TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                topic_tags_json TEXT NOT NULL DEFAULT '[]',
                completed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE students (
                id TEXT PRIMARY KEY,
                full_name TEXT NOT NULL,
                archived_at TEXT
            );
            CREATE TABLE attendance_records (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                student_id TEXT NOT NULL,
                status TEXT NOT NULL,
                note TEXT,
                recorded_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(session_id, student_id)
            );",
        )
        .expect("schema");
        conn
    }

    #[test]
    fn upsert_attendance_updates_existing_row() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, class_id, session_date, title, topic_tags_json, completed, created_at, updated_at)
             VALUES ('session-1', 'class-1', '2026-05-20', 'Lesson 1', '[]', 0, 't', 't')",
            [],
        )
        .expect("insert session");
        conn.execute(
            "INSERT INTO students (id, full_name, archived_at) VALUES ('student-1', 'A', NULL)",
            [],
        )
        .expect("insert student");

        let first_id =
            upsert_attendance_record(&conn, "session-1", "student-1", "present", Some("on time"))
                .expect("first upsert");
        let second_id =
            upsert_attendance_record(&conn, "session-1", "student-1", "absent", Some("sick"))
                .expect("second upsert");

        assert_ne!(first_id, second_id, "upsert generates fresh ids per call");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM attendance_records", [], |row| {
                row.get(0)
            })
            .expect("count rows");
        assert_eq!(count, 1, "should keep a single row per (session, student)");

        let (status, note): (String, Option<String>) = conn
            .query_row(
                "SELECT status, note FROM attendance_records WHERE session_id = 'session-1' AND student_id = 'student-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("load updated row");
        assert_eq!(status, "absent");
        assert_eq!(note.as_deref(), Some("sick"));
    }

    #[test]
    fn set_session_completion_toggles_completed_flag() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, class_id, session_date, title, topic_tags_json, completed, created_at, updated_at)
             VALUES ('session-2', 'class-1', '2026-05-20', 'Lesson 2', '[]', 0, 't', 't')",
            [],
        )
        .expect("insert session");

        set_session_completion(&conn, "session-2", true).expect("mark completed");
        let completed: bool = conn
            .query_row(
                "SELECT completed FROM sessions WHERE id = 'session-2'",
                [],
                |row| row.get(0),
            )
            .expect("query completed");
        assert!(completed);
    }
}
