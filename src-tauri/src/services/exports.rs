use crate::services::hierarchy::{Database, DbState, YearPlanLesson};
use crate::services::planning::year_plan_generator::get_year_plan_lessons;
use crate::services::reports::report_generator::load_report;
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

const WEEKLY_BACKUP_ENABLED_KEY: &str = "backup.weekly.enabled";
const WEEKLY_BACKUP_LAST_AT_KEY: &str = "backup.weekly.last_export_at";
const WEEKLY_BACKUP_INTERVAL_DAYS: i64 = 7;

fn export_dir(state: &Database) -> Result<PathBuf, String> {
    let dir = state.data_dir().join("exports");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBackupExport {
    pub path: String,
    pub file_name: String,
    pub created_at: String,
    pub file_size_bytes: u64,
}

fn sqlite_literal_path(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn create_database_backup(
    state: &Database,
    file_prefix: &str,
) -> Result<DatabaseBackupExport, String> {
    let dir = export_dir(state)?;
    let created_at = Utc::now().to_rfc3339();
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S-%3f").to_string();
    let file_name = format!("{file_prefix}-{timestamp}.sqlite3");
    let path = dir.join(&file_name);

    state.with_conn(|conn| {
        // Attempt to checkpoint WAL before snapshot; harmless for non-WAL modes.
        conn.execute_batch("PRAGMA wal_checkpoint(FULL);").ok();
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        conn.execute_batch(&format!("VACUUM INTO '{}';", sqlite_literal_path(&path)))
            .map_err(|e| e.to_string())
    })?;

    let file_size_bytes = fs::metadata(&path).map_err(|e| e.to_string())?.len();
    Ok(DatabaseBackupExport {
        path: path.to_string_lossy().to_string(),
        file_name,
        created_at,
        file_size_bytes,
    })
}

fn read_setting_value_json(
    conn: &rusqlite::Connection,
    key: &str,
) -> Result<Option<String>, String> {
    match conn.query_row(
        "SELECT value_json FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn write_setting_value_json(
    conn: &rusqlite::Connection,
    key: &str,
    value_json: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        params![key, value_json, Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_setting_bool(value_json: &str) -> Option<bool> {
    serde_json::from_str::<bool>(value_json).ok().or_else(|| {
        match value_json.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    })
}

fn parse_setting_string(value_json: &str) -> Option<String> {
    serde_json::from_str::<String>(value_json).ok().or_else(|| {
        let trimmed = value_json.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[tauri::command]
pub async fn export_database_backup(state: DbState<'_>) -> Result<DatabaseBackupExport, String> {
    create_database_backup(&state, "edutrack-backup")
}

#[tauri::command]
pub async fn run_weekly_backup_if_due(
    state: DbState<'_>,
) -> Result<Option<DatabaseBackupExport>, String> {
    let now = Utc::now();
    let (enabled, last_backup_at) = state.with_conn(|conn| {
        let enabled = read_setting_value_json(conn, WEEKLY_BACKUP_ENABLED_KEY)?
            .as_deref()
            .and_then(parse_setting_bool)
            .unwrap_or(true);
        let last_backup_at = read_setting_value_json(conn, WEEKLY_BACKUP_LAST_AT_KEY)?
            .as_deref()
            .and_then(parse_setting_string);
        Ok((enabled, last_backup_at))
    })?;

    if !enabled {
        return Ok(None);
    }

    let due = match last_backup_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
    {
        Some(last) => {
            now.signed_duration_since(last.with_timezone(&Utc))
                >= Duration::days(WEEKLY_BACKUP_INTERVAL_DAYS)
        }
        None => true,
    };

    if !due {
        return Ok(None);
    }

    let backup = create_database_backup(&state, "edutrack-backup-weekly")?;
    let created_at_json = serde_json::to_string(&backup.created_at).map_err(|e| e.to_string())?;
    state.with_conn(|conn| {
        write_setting_value_json(conn, WEEKLY_BACKUP_ENABLED_KEY, "true")?;
        write_setting_value_json(conn, WEEKLY_BACKUP_LAST_AT_KEY, &created_at_json)?;
        Ok(())
    })?;

    Ok(Some(backup))
}

#[tauri::command]
pub async fn export_year_plan_csv(
    state: DbState<'_>,
    year_plan_id: String,
) -> Result<String, String> {
    let dir = export_dir(&state)?;
    let lessons: Vec<YearPlanLesson> = get_year_plan_lessons(state, year_plan_id.clone()).await?;
    let mut csv = "date,weekday,sequence,title,objective,status,is_buffer\n".to_string();
    for lesson in lessons {
        csv.push_str(&format!(
            "{},{},{},\"{}\",\"{}\",{},{}\n",
            lesson.teaching_date,
            lesson.weekday,
            lesson.sequence_number,
            lesson.lesson_title.replace('"', "\"\""),
            lesson
                .lesson_objective
                .unwrap_or_default()
                .replace('"', "\"\""),
            lesson.status,
            lesson.is_buffer
        ));
    }
    let path = dir.join(format!("{year_plan_id}.csv"));
    fs::write(&path, csv).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn export_student_report_txt(
    state: DbState<'_>,
    report_id: String,
    include_teacher_advice: bool,
) -> Result<String, String> {
    let dir = export_dir(&state)?;
    state.with_conn(|conn| {
        let report = load_report(conn, &report_id)?;
        let mut body = report.report_text;
        if include_teacher_advice {
            if let Some(advice) = report.teacher_advice_text {
                body.push_str("\n\nTeacher advice\n");
                body.push_str(&advice);
            }
        }
        let path = dir.join(format!("{report_id}.txt"));
        fs::write(&path, body).map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().to_string())
    })
}

#[tauri::command]
pub async fn export_school_data_json(
    state: DbState<'_>,
    school_id: String,
    include_private_notes: bool,
) -> Result<String, String> {
    let dir = export_dir(&state)?;
    state.with_conn(|conn| {
        let schools: Vec<serde_json::Value> = query_json(
            conn,
            "SELECT id, name, country_code, region_code, timezone, academic_year_label FROM schools WHERE id = ?1",
            &school_id,
        )?;
        let levels: Vec<serde_json::Value> = query_json(
            conn,
            "SELECT id, school_id, name, code, academic_year_start, academic_year_end, region_code, planning_status FROM levels WHERE school_id = ?1 AND archived_at IS NULL",
            &school_id,
        )?;
        let classes: Vec<serde_json::Value> = query_json(
            conn,
            "SELECT id, school_id, level_id, name, subject_name, schedule_pattern_summary, year_plan_id FROM classes WHERE school_id = ?1 AND archived_at IS NULL",
            &school_id,
        )?;
        let student_sql = if include_private_notes {
            "SELECT id, class_id, school_id, level_id, full_name, preferred_name, student_code, notes_private FROM students WHERE school_id = ?1 AND archived_at IS NULL"
        } else {
            "SELECT id, class_id, school_id, level_id, full_name, preferred_name, student_code, NULL AS notes_private FROM students WHERE school_id = ?1 AND archived_at IS NULL"
        };
        let students = query_json(conn, student_sql, &school_id)?;
        let payload = json!({
            "exportType": "school_data",
            "schoolId": school_id,
            "includePrivateNotes": include_private_notes,
            "schools": schools,
            "levels": levels,
            "classes": classes,
            "students": students
        });
        let path = dir.join(format!("school-export-{}.json", payload["schoolId"].as_str().unwrap_or("school")));
        fs::write(
            &path,
            serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().to_string())
    })
}

fn query_json(
    conn: &rusqlite::Connection,
    sql: &str,
    id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let column_names: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect();
    let rows = stmt
        .query_map(params![id], |row| {
            let mut object = serde_json::Map::new();
            for (index, name) in column_names.iter().enumerate() {
                let value: Option<String> = row.get(index)?;
                object.insert(
                    name.clone(),
                    value.map_or(serde_json::Value::Null, serde_json::Value::String),
                );
            }
            Ok(serde_json::Value::Object(object))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{create_database_backup, export_dir};
    use crate::services::hierarchy::Database;
    use rusqlite::Connection;
    use std::sync::Arc;

    #[test]
    fn export_dir_resolved_via_data_dir() {
        let tmp = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&tmp).unwrap();
        let db = Arc::new(Database::new(tmp.join("test.db"), tmp.join("data")).unwrap());
        let dir = export_dir(db.as_ref()).unwrap();
        assert!(dir.ends_with("exports"));
        assert!(dir.exists());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn database_backup_can_be_opened_and_contains_data() {
        let tmp = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&tmp).unwrap();
        let db = Database::new(tmp.join("test.db"), tmp.join("data")).unwrap();
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO schools (id, name, timezone, created_at, updated_at) VALUES ('school-1', 'School', 'UTC', 't', 't')",
                [],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })
        .unwrap();

        let backup = create_database_backup(&db, "test-backup").unwrap();
        let backup_conn = Connection::open(&backup.path).unwrap();
        let count: i64 = backup_conn
            .query_row(
                "SELECT COUNT(*) FROM schools WHERE id = 'school-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
