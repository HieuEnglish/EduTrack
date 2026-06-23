use base64::Engine;
use chrono::Utc;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

pub type DbState<'a> = State<'a, std::sync::Arc<Database>>;

pub struct Database {
    conn: Mutex<Connection>,
    data_path: PathBuf,
}

impl Database {
    pub fn new(db_path: impl AsRef<Path>, data_path: impl AsRef<Path>) -> Result<Self, String> {
        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(|e| e.to_string())?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| e.to_string())?;

        let has_schema: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schools'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if has_schema == 0 {
            conn.execute_batch(include_str!("../db/schema.sql"))
                .map_err(|e| e.to_string())?;
        }

        apply_schema_migrations(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            data_path: data_path.as_ref().to_path_buf(),
        })
    }

    pub fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        f(&mut conn)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_path
    }

    pub fn uploads_dir(&self, subdir: &str) -> PathBuf {
        self.data_path.join("uploads").join(subdir)
    }
}

type MigrationFn = fn(&Connection) -> Result<(), String>;

struct Migration {
    version: &'static str,
    apply: MigrationFn,
}

const MIGRATIONS: [Migration; 6] = [
    Migration {
        version: "001_add_student_age_gender",
        apply: migration_001_add_student_age_gender,
    },
    Migration {
        version: "002_add_period_minutes",
        apply: migration_002_add_period_minutes,
    },
    Migration {
        version: "003_planning_foundation",
        apply: migration_003_planning_foundation,
    },
    Migration {
        version: "004_detailed_plan_attachment",
        apply: migration_004_detailed_plan_attachment,
    },
    Migration {
        version: "005_curriculum_units_month_label",
        apply: migration_005_curriculum_units_month_label,
    },
    Migration {
        version: "006_agent_insights_index",
        apply: migration_006_agent_insights_index,
    },
];

fn apply_schema_migrations(conn: &Connection) -> Result<(), String> {
    ensure_schema_migrations_table(conn)?;
    for migration in MIGRATIONS {
        if is_migration_applied(conn, migration.version)? {
            continue;
        }
        (migration.apply)(conn)?;
        mark_migration_applied(conn, migration.version)?;
    }
    Ok(())
}

fn ensure_schema_migrations_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())
}

fn is_migration_applied(conn: &Connection, version: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM schema_migrations WHERE version = ?1",
        params![version],
        |_| Ok(()),
    )
    .map(|_| true)
    .or_else(|e| {
        if let rusqlite::Error::QueryReturnedNoRows = e {
            Ok(false)
        } else {
            Err(e.to_string())
        }
    })
}

fn mark_migration_applied(conn: &Connection, version: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        params![version, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn migration_001_add_student_age_gender(conn: &Connection) -> Result<(), String> {
    add_column_if_missing(conn, "students", "age", "INTEGER")?;
    add_column_if_missing(conn, "students", "gender", "TEXT")?;
    Ok(())
}

fn migration_002_add_period_minutes(conn: &Connection) -> Result<(), String> {
    add_column_if_missing(conn, "classes", "period_minutes", "INTEGER NOT NULL DEFAULT 45")
}

fn migration_003_planning_foundation(conn: &Connection) -> Result<(), String> {
    add_column_if_missing(conn, "levels", "default_syllabus_id", "TEXT")?;
    add_column_if_missing(conn, "levels", "default_calendar_id", "TEXT")?;
    add_column_if_missing(
        conn,
        "levels",
        "planning_status",
        "TEXT NOT NULL DEFAULT 'draft'",
    )?;
    add_column_if_missing(conn, "classes", "year_plan_id", "TEXT")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS academic_calendars (
            id TEXT PRIMARY KEY,
            school_id TEXT NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
            level_id TEXT REFERENCES levels(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            academic_year_start TEXT NOT NULL,
            academic_year_end TEXT NOT NULL,
            region_code TEXT NOT NULL,
            timezone TEXT NOT NULL,
            is_default INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_academic_calendars_school_level ON academic_calendars(school_id, level_id);

        CREATE TABLE IF NOT EXISTS calendar_closure_days (
            id TEXT PRIMARY KEY,
            calendar_id TEXT NOT NULL REFERENCES academic_calendars(id) ON DELETE CASCADE,
            closure_date TEXT NOT NULL,
            closure_type TEXT NOT NULL,
            title TEXT,
            source_provider TEXT,
            created_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_calendar_closure_unique ON calendar_closure_days(calendar_id, closure_date, closure_type);

        CREATE TABLE IF NOT EXISTS syllabus_documents (
            id TEXT PRIMARY KEY,
            school_id TEXT NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
            level_id TEXT NOT NULL REFERENCES levels(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            version_label TEXT,
            source_file_attachment_id TEXT NOT NULL,
            parser_status TEXT NOT NULL DEFAULT 'pending',
            llm_extraction_status TEXT NOT NULL DEFAULT 'not_started',
            coverage_notes TEXT,
            is_active INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            archived_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_syllabus_documents_level_id ON syllabus_documents(level_id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_syllabus_documents_one_active ON syllabus_documents(level_id) WHERE is_active = 1 AND archived_at IS NULL;

        CREATE TABLE IF NOT EXISTS curriculum_units (
            id TEXT PRIMARY KEY,
            syllabus_id TEXT NOT NULL REFERENCES syllabus_documents(id) ON DELETE CASCADE,
            unit_code TEXT,
            title TEXT NOT NULL,
            description TEXT,
            recommended_sequence INTEGER NOT NULL,
            estimated_lessons INTEGER,
            estimated_weeks INTEGER,
            assessment_hint TEXT,
            is_required INTEGER NOT NULL DEFAULT 1,
            month_label TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_curriculum_units_syllabus_id ON curriculum_units(syllabus_id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_curriculum_units_sequence ON curriculum_units(syllabus_id, recommended_sequence);",
    )
    .map_err(|e| e.to_string())?;

    // Existing databases may not have this column; safe no-op when already present.
    conn.execute_batch("ALTER TABLE curriculum_units ADD COLUMN month_label TEXT")
        .ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS class_schedule_rules (
            id TEXT PRIMARY KEY,
            class_id TEXT NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
            weekday TEXT NOT NULL,
            start_time TEXT,
            end_time TEXT,
            period_label TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_class_schedule_unique ON class_schedule_rules(class_id, weekday, COALESCE(period_label, '')) WHERE is_active = 1;

        CREATE TABLE IF NOT EXISTS year_plans (
            id TEXT PRIMARY KEY,
            school_id TEXT NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
            level_id TEXT NOT NULL REFERENCES levels(id) ON DELETE CASCADE,
            class_id TEXT REFERENCES classes(id) ON DELETE CASCADE,
            syllabus_id TEXT NOT NULL REFERENCES syllabus_documents(id),
            calendar_id TEXT NOT NULL REFERENCES academic_calendars(id),
            region_code TEXT NOT NULL,
            plan_scope TEXT NOT NULL,
            generation_status TEXT NOT NULL,
            generation_summary TEXT,
            total_teaching_days INTEGER NOT NULL DEFAULT 0,
            total_planned_lessons INTEGER NOT NULL DEFAULT 0,
            holiday_days_excluded INTEGER NOT NULL DEFAULT 0,
            buffer_days_reserved INTEGER NOT NULL DEFAULT 0,
            plan_snapshot_json TEXT NOT NULL,
            generated_at TEXT,
            published_at TEXT,
            superseded_by_plan_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_year_plans_level_id ON year_plans(level_id);
        CREATE INDEX IF NOT EXISTS idx_year_plans_class_id ON year_plans(class_id);

        CREATE TABLE IF NOT EXISTS year_plan_lessons (
            id TEXT PRIMARY KEY,
            year_plan_id TEXT NOT NULL REFERENCES year_plans(id) ON DELETE CASCADE,
            teaching_date TEXT NOT NULL,
            weekday TEXT NOT NULL,
            sequence_number INTEGER NOT NULL,
            curriculum_unit_id TEXT REFERENCES curriculum_units(id),
            lesson_title TEXT NOT NULL,
            lesson_objective TEXT,
            detailed_plan_attached INTEGER NOT NULL DEFAULT 0,
            coverage_weight REAL,
            is_buffer INTEGER NOT NULL DEFAULT 0,
            is_holiday_adjusted INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'planned',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_year_plan_lessons_plan_id ON year_plan_lessons(year_plan_id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_year_plan_lessons_plan_date ON year_plan_lessons(year_plan_id, teaching_date);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_year_plan_lessons_plan_seq ON year_plan_lessons(year_plan_id, sequence_number);",
    )
    .map_err(|e| e.to_string())
}

fn migration_004_detailed_plan_attachment(conn: &Connection) -> Result<(), String> {
    add_column_if_missing(
        conn,
        "year_plan_lessons",
        "detailed_plan_attached",
        "INTEGER NOT NULL DEFAULT 0",
    )
}

fn migration_005_curriculum_units_month_label(conn: &Connection) -> Result<(), String> {
    add_column_if_missing(conn, "curriculum_units", "month_label", "TEXT")
}

fn migration_006_agent_insights_index(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_insights_class_created ON agent_insights(class_id, created_at DESC)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn now() -> String {
    Utc::now().to_rfc3339()
}

pub fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if !columns.iter().any(|name| name == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct School {
    pub id: String,
    pub name: String,
    pub logo_path: Option<String>,
    pub country_code: Option<String>,
    pub region_code: Option<String>,
    pub timezone: String,
    pub academic_year_label: Option<String>,
    pub default_report_word_count: i32,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    pub id: String,
    pub school_id: String,
    pub name: String,
    pub code: Option<String>,
    pub display_order: i32,
    pub academic_year_start: String,
    pub academic_year_end: String,
    pub region_code: String,
    pub timezone: String,
    pub default_syllabus_id: Option<String>,
    pub default_calendar_id: Option<String>,
    pub planning_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Class {
    pub id: String,
    pub school_id: String,
    pub level_id: String,
    pub name: String,
    pub subject_name: Option<String>,
    pub academic_year_label: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub period_minutes: i32,
    pub max_students: i32,
    pub schedule_pattern_summary: Option<String>,
    pub year_plan_id: Option<String>,
    pub teacher_notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Student {
    pub id: String,
    pub class_id: String,
    pub school_id: String,
    pub level_id: String,
    pub full_name: String,
    pub preferred_name: Option<String>,
    pub student_code: Option<String>,
    pub age: Option<i32>,
    pub gender: Option<String>,
    pub avatar_seed: Option<String>,
    pub notes_private: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyllabusDocument {
    pub id: String,
    pub school_id: String,
    pub level_id: String,
    pub title: String,
    pub version_label: Option<String>,
    pub source_file_attachment_id: String,
    pub parser_status: String,
    pub llm_extraction_status: String,
    pub coverage_notes: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumUnit {
    pub id: String,
    pub syllabus_id: String,
    pub unit_code: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub recommended_sequence: i32,
    pub estimated_lessons: i32,
    pub estimated_weeks: i32,
    pub assessment_hint: Option<String>,
    pub is_required: bool,
    #[serde(default)]
    pub month_label: Option<String>,
    #[serde(default)]
    pub schedule_slot_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct YearPlan {
    pub id: String,
    pub school_id: String,
    pub level_id: String,
    pub class_id: Option<String>,
    pub syllabus_id: String,
    pub calendar_id: String,
    pub region_code: String,
    pub plan_scope: String,
    pub generation_status: String,
    pub generation_summary: Option<String>,
    pub total_teaching_days: i32,
    pub total_planned_lessons: i32,
    pub holiday_days_excluded: i32,
    pub buffer_days_reserved: i32,
    pub plan_snapshot_json: String,
    pub generated_at: Option<String>,
    pub published_at: Option<String>,
    pub superseded_by_plan_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct YearPlanLesson {
    pub id: String,
    pub year_plan_id: String,
    pub teaching_date: String,
    pub weekday: String,
    pub sequence_number: i32,
    pub curriculum_unit_id: Option<String>,
    pub lesson_title: String,
    pub lesson_objective: Option<String>,
    pub detailed_plan_attached: bool,
    pub coverage_weight: Option<f64>,
    pub is_buffer: bool,
    pub is_holiday_adjusted: bool,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSchoolDataResult {
    pub schools_deleted: i32,
    pub levels_deleted: i32,
    pub classes_deleted: i32,
    pub students_deleted: i32,
    pub year_plans_deleted: i32,
    pub year_plan_lessons_deleted: i32,
}

fn school_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<School> {
    Ok(School {
        id: row.get(0)?,
        name: row.get(1)?,
        logo_path: row.get(2)?,
        country_code: row.get(3)?,
        region_code: row.get(4)?,
        timezone: row.get(5)?,
        academic_year_label: row.get(6)?,
        default_report_word_count: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        archived_at: row.get(10)?,
    })
}

fn level_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Level> {
    Ok(Level {
        id: row.get(0)?,
        school_id: row.get(1)?,
        name: row.get(2)?,
        code: row.get(3)?,
        display_order: row.get(4)?,
        academic_year_start: row.get(5)?,
        academic_year_end: row.get(6)?,
        region_code: row.get(7)?,
        timezone: row.get(8)?,
        default_syllabus_id: row.get(9)?,
        default_calendar_id: row.get(10)?,
        planning_status: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        archived_at: row.get(14)?,
    })
}

pub fn get_level(conn: &Connection, level_id: &str) -> Result<Level, String> {
    conn.query_row(
        "SELECT id, school_id, name, code, display_order, academic_year_start, academic_year_end, region_code, timezone, default_syllabus_id, default_calendar_id, planning_status, created_at, updated_at, archived_at FROM levels WHERE id = ?1",
        params![level_id],
        level_from_row,
    )
    .map_err(|e| e.to_string())
}

pub fn get_class(conn: &Connection, class_id: &str) -> Result<Class, String> {
    conn.query_row(
        "SELECT id, school_id, level_id, name, subject_name, academic_year_label, start_date, end_date, period_minutes, max_students, schedule_pattern_summary, year_plan_id, teacher_notes, created_at, updated_at, archived_at FROM classes WHERE id = ?1",
        params![class_id],
        |row| {
            Ok(Class {
                id: row.get(0)?,
                school_id: row.get(1)?,
                level_id: row.get(2)?,
                name: row.get(3)?,
                subject_name: row.get(4)?,
                academic_year_label: row.get(5)?,
                start_date: row.get(6)?,
                end_date: row.get(7)?,
                period_minutes: row.get(8)?,
                max_students: row.get(9)?,
                schedule_pattern_summary: row.get(10)?,
                year_plan_id: row.get(11)?,
                teacher_notes: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
                archived_at: row.get(15)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_school(state: DbState<'_>, mut school: School) -> Result<String, String> {
    state.with_conn(|conn| {
        if school.id.is_empty() {
            school.id = new_id("school");
        }
        let timestamp = now();
        if school.created_at.is_empty() {
            school.created_at = timestamp.clone();
        }
        school.updated_at = timestamp;
        if school.timezone.is_empty() {
            school.timezone = "UTC".to_string();
        }
        if school.default_report_word_count <= 0 {
            school.default_report_word_count = 200;
        }
        conn.execute(
            "INSERT INTO schools (id, name, logo_path, country_code, region_code, timezone, academic_year_label, default_report_word_count, created_at, updated_at, archived_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                school.id,
                school.name,
                school.logo_path,
                school.country_code,
                school.region_code,
                school.timezone,
                school.academic_year_label,
                school.default_report_word_count,
                school.created_at,
                school.updated_at,
                school.archived_at
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(school.id)
    })
}

#[tauri::command]
pub async fn get_schools(state: DbState<'_>) -> Result<Vec<School>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, name, logo_path, country_code, region_code, timezone, academic_year_label, default_report_word_count, created_at, updated_at, archived_at FROM schools WHERE archived_at IS NULL ORDER BY name")
            .map_err(|e| e.to_string())?;
        let result = stmt.query_map([], school_from_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn archive_school(state: DbState<'_>, school_id: String) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE schools SET archived_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now(), school_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub async fn delete_school_data(
    state: DbState<'_>,
    school_id: String,
) -> Result<DeleteSchoolDataResult, String> {
    state.with_conn(|conn| {
        let year_plan_lessons_deleted = conn.execute(
            "DELETE FROM year_plan_lessons
             WHERE year_plan_id IN (
               SELECT yp.id
               FROM year_plans yp
               LEFT JOIN classes c ON yp.class_id = c.id
               WHERE yp.school_id = ?1
                  OR c.school_id = ?1
                  OR yp.level_id IN (SELECT id FROM levels WHERE school_id = ?1)
             )",
            params![school_id],
        ).map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM attendance_records WHERE session_id IN (SELECT id FROM sessions WHERE class_id IN (SELECT id FROM classes WHERE school_id = ?1))", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM assessment_scores WHERE assessment_id IN (SELECT id FROM assessments WHERE class_id IN (SELECT id FROM classes WHERE school_id = ?1))", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM student_reports WHERE class_id IN (SELECT id FROM classes WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM agent_insights WHERE class_id IN (SELECT id FROM classes WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM class_agent_assignments WHERE class_id IN (SELECT id FROM classes WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM class_schedule_rules WHERE class_id IN (SELECT id FROM classes WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM sessions WHERE class_id IN (SELECT id FROM classes WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM assessments WHERE class_id IN (SELECT id FROM classes WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM student_characters WHERE student_id IN (SELECT id FROM students WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        let year_plans_deleted = conn.execute(
            "DELETE FROM year_plans
             WHERE school_id = ?1
                OR class_id IN (SELECT id FROM classes WHERE school_id = ?1)
                OR level_id IN (SELECT id FROM levels WHERE school_id = ?1)",
            params![school_id],
        ).map_err(|e| e.to_string())?;
        let students_deleted = conn.execute("DELETE FROM students WHERE school_id = ?1", params![school_id])
            .map_err(|e| e.to_string())?;
        let classes_deleted = conn.execute("DELETE FROM classes WHERE school_id = ?1", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM curriculum_units WHERE syllabus_id IN (SELECT id FROM syllabus_documents WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM calendar_closure_days WHERE calendar_id IN (SELECT id FROM academic_calendars WHERE school_id = ?1)", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM academic_calendars WHERE school_id = ?1", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM syllabus_documents WHERE school_id = ?1", params![school_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM attachments WHERE owner_type = 'school' AND owner_id = ?1", params![school_id])
            .map_err(|e| e.to_string())?;
        let levels_deleted = conn.execute("DELETE FROM levels WHERE school_id = ?1", params![school_id])
            .map_err(|e| e.to_string())?;
        let schools_deleted = conn.execute("DELETE FROM schools WHERE id = ?1", params![school_id])
            .map_err(|e| e.to_string())?;
        Ok(DeleteSchoolDataResult {
            schools_deleted: schools_deleted as i32,
            levels_deleted: levels_deleted as i32,
            classes_deleted: classes_deleted as i32,
            students_deleted: students_deleted as i32,
            year_plans_deleted: year_plans_deleted as i32,
            year_plan_lessons_deleted: year_plan_lessons_deleted as i32,
        })
    })
}

#[tauri::command]
pub async fn create_level(state: DbState<'_>, mut level: Level) -> Result<String, String> {
    state.with_conn(|conn| {
        if level.id.is_empty() {
            level.id = new_id("level");
        }
        let timestamp = now();
        if level.created_at.is_empty() {
            level.created_at = timestamp.clone();
        }
        level.updated_at = timestamp;
        if level.timezone.is_empty() {
            level.timezone = "UTC".to_string();
        }
        if level.planning_status.is_empty() {
            level.planning_status = "draft".to_string();
        }
        conn.execute(
            "INSERT INTO levels (id, school_id, name, code, display_order, academic_year_start, academic_year_end, region_code, timezone, default_syllabus_id, default_calendar_id, planning_status, created_at, updated_at, archived_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                level.id,
                level.school_id,
                level.name,
                level.code,
                level.display_order,
                level.academic_year_start,
                level.academic_year_end,
                level.region_code,
                level.timezone,
                level.default_syllabus_id,
                level.default_calendar_id,
                level.planning_status,
                level.created_at,
                level.updated_at,
                level.archived_at
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(level.id)
    })
}

#[tauri::command]
pub async fn get_levels_by_school(
    state: DbState<'_>,
    school_id: String,
) -> Result<Vec<Level>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, school_id, name, code, display_order, academic_year_start, academic_year_end, region_code, timezone, default_syllabus_id, default_calendar_id, planning_status, created_at, updated_at, archived_at FROM levels WHERE school_id = ?1 AND archived_at IS NULL ORDER BY display_order, name")
            .map_err(|e| e.to_string())?;
        let result = stmt.query_map(params![school_id], level_from_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn create_class(state: DbState<'_>, mut class: Class) -> Result<String, String> {
    state.with_conn(|conn| {
        if class.id.is_empty() {
            class.id = new_id("class");
        }
        let timestamp = now();
        if class.created_at.is_empty() {
            class.created_at = timestamp.clone();
        }
        class.updated_at = timestamp;
        if class.period_minutes <= 0 {
            class.period_minutes = 45;
        }
        if class.max_students <= 0 {
            class.max_students = 50;
        }
        conn.execute(
            "INSERT INTO classes (id, school_id, level_id, name, subject_name, academic_year_label, start_date, end_date, period_minutes, max_students, schedule_pattern_summary, year_plan_id, teacher_notes, created_at, updated_at, archived_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                class.id,
                class.school_id,
                class.level_id,
                class.name,
                class.subject_name,
                class.academic_year_label,
                class.start_date,
                class.end_date,
                class.period_minutes,
                class.max_students,
                class.schedule_pattern_summary,
                class.year_plan_id,
                class.teacher_notes,
                class.created_at,
                class.updated_at,
                class.archived_at
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(class.id)
    })
}

#[tauri::command]
pub async fn get_classes_by_level(
    state: DbState<'_>,
    level_id: String,
) -> Result<Vec<Class>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, school_id, level_id, name, subject_name, academic_year_label, start_date, end_date, period_minutes, max_students, schedule_pattern_summary, year_plan_id, teacher_notes, created_at, updated_at, archived_at FROM classes WHERE level_id = ?1 AND archived_at IS NULL ORDER BY name")
            .map_err(|e| e.to_string())?;
        let result = stmt.query_map(params![level_id], |row| {
            Ok(Class {
                id: row.get(0)?,
                school_id: row.get(1)?,
                level_id: row.get(2)?,
                name: row.get(3)?,
                subject_name: row.get(4)?,
                academic_year_label: row.get(5)?,
                start_date: row.get(6)?,
                end_date: row.get(7)?,
                period_minutes: row.get(8)?,
                max_students: row.get(9)?,
                schedule_pattern_summary: row.get(10)?,
                year_plan_id: row.get(11)?,
                teacher_notes: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
                archived_at: row.get(15)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn archive_class(state: DbState<'_>, class_id: String) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE classes SET archived_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now(), class_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub async fn create_student(state: DbState<'_>, mut student: Student) -> Result<String, String> {
    state.with_conn(|conn| {
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM students WHERE class_id = ?1 AND archived_at IS NULL",
                params![student.class_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let max_students: i32 = conn
            .query_row(
                "SELECT max_students FROM classes WHERE id = ?1",
                params![student.class_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count >= max_students {
            return Err(format!("class is at its maximum of {max_students} students"));
        }

        if student.id.is_empty() {
            student.id = new_id("student");
        }
        let timestamp = now();
        if student.created_at.is_empty() {
            student.created_at = timestamp.clone();
        }
        student.updated_at = timestamp;
        conn.execute(
            "INSERT INTO students (id, class_id, school_id, level_id, full_name, preferred_name, student_code, age, gender, avatar_seed, notes_private, created_at, updated_at, archived_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                student.id,
                student.class_id,
                student.school_id,
                student.level_id,
                student.full_name,
                student.preferred_name,
                student.student_code,
                student.age,
                student.gender,
                student.avatar_seed,
                student.notes_private,
                student.created_at,
                student.updated_at,
                student.archived_at
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(student.id)
    })
}

#[tauri::command]
pub async fn update_student_note(
    state: DbState<'_>,
    student_id: String,
    note: Option<String>,
) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE students SET notes_private = ?1, updated_at = ?2 WHERE id = ?3",
            params![note, now(), student_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub async fn archive_student(state: DbState<'_>, student_id: String) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE students SET archived_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now(), student_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub async fn get_students_by_class(
    state: DbState<'_>,
    class_id: String,
) -> Result<Vec<Student>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, class_id, school_id, level_id, full_name, preferred_name, student_code, age, gender, avatar_seed, notes_private, created_at, updated_at, archived_at FROM students WHERE class_id = ?1 AND archived_at IS NULL ORDER BY full_name")
            .map_err(|e| e.to_string())?;
        let result = stmt.query_map(params![class_id], |row| {
            Ok(Student {
                id: row.get(0)?,
                class_id: row.get(1)?,
                school_id: row.get(2)?,
                level_id: row.get(3)?,
                full_name: row.get(4)?,
                preferred_name: row.get(5)?,
                student_code: row.get(6)?,
                age: row.get(7)?,
                gender: row.get(8)?,
                avatar_seed: row.get(9)?,
                notes_private: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
                archived_at: row.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn get_teacher_profile(state: DbState<'_>) -> Result<Option<String>, String> {
    state.with_conn(|conn| {
        match conn.query_row(
            "SELECT value_json FROM settings WHERE key = 'teacher.profile'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    })
}

#[tauri::command]
pub async fn save_teacher_profile(
    state: DbState<'_>,
    profile_json: String,
) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES ('teacher.profile', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![profile_json, now()],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub async fn save_image_file(
    state: DbState<'_>,
    file_data: Vec<u8>,
    file_name: String,
) -> Result<String, String> {
    let images_dir = state.uploads_dir("images");
    std::fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;
    let safe_name = format!("{}_{}", &new_id("img")[..20], file_name.replace(['/', '\\', ':'], "_"));
    let file_path = images_dir.join(&safe_name);
    std::fs::write(&file_path, &file_data).map_err(|e| e.to_string())?;
    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn read_image_file(path: String) -> Result<String, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mime = match path.rsplit_once('.').unwrap_or(("", "")).1.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&data)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_has_prefix() {
        let id = new_id("school");
        assert!(id.starts_with("school-"));
        assert!(id.len() > 10);
    }

    #[test]
    fn new_id_unique() {
        let a = new_id("test");
        let b = new_id("test");
        assert_ne!(a, b);
    }

    #[test]
    fn now_is_rfc3339() {
        let ts = now();
        assert!(ts.contains('T'));
        assert!(ts.contains('Z') || ts.contains('+'));
    }
}
