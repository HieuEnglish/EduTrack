use crate::services::hierarchy::{
    get_level, new_id, now, CurriculumUnit, DbState, SyllabusDocument,
};

use pdf_extract::extract_text;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyllabusReview {
    pub syllabus: SyllabusDocument,
    pub units: Vec<CurriculumUnit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedCurriculumUnit {
    pub id: Option<String>,
    pub title: String,
    pub unit_code: Option<String>,
    pub description: Option<String>,
    pub estimated_lessons: Option<i32>,
    pub estimated_weeks: Option<i32>,
    pub assessment_hint: Option<String>,
    pub is_required: Option<bool>,
    pub month_label: Option<String>,
}

#[tauri::command]
pub async fn upload_syllabus(
    state: DbState<'_>,
    level_id: String,
    file_name: String,
    file_data: Vec<u8>,
) -> Result<String, String> {
    let upload_dir = state.uploads_dir("syllabi").join(&level_id);
    fs::create_dir_all(&upload_dir).map_err(|e| e.to_string())?;
    state.with_conn(|conn| {
        let level = get_level(conn, &level_id)?;
        let safe_name = file_name.replace(['/', '\\', ':'], "_");
        let file_path = upload_dir.join(&safe_name);
        fs::write(&file_path, file_data).map_err(|e| e.to_string())?;

        let extracted_text = extract_text_from_file(&file_path)?;
        let syllabus_id = new_id("syllabus");
        let timestamp = now();
        conn.execute(
            "UPDATE syllabus_documents SET is_active = 0 WHERE level_id = ?1 AND archived_at IS NULL",
            params![level_id],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO syllabus_documents (id, school_id, level_id, title, version_label, source_file_attachment_id, parser_status, llm_extraction_status, coverage_notes, is_active, created_at, updated_at, archived_at)
             VALUES (?1, ?2, ?3, ?4, '1.0', ?5, 'parsed', 'ready', ?6, 1, ?7, ?8, NULL)",
            params![
                syllabus_id,
                level.school_id,
                level_id,
                title_from_file_name(&safe_name),
                file_path.to_string_lossy().to_string(),
                extracted_text,
                timestamp,
                timestamp
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(syllabus_id)
    })
}

fn extract_text_from_file(path: &PathBuf) -> Result<String, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "pdf" => extract_text(path).map_err(|e| format!("failed to extract PDF text: {e}")),
        "txt" | "md" | "csv" => fs::read_to_string(path).map_err(|e| e.to_string()),
        "doc" | "docx" => {
            Err("DOC/DOCX parsing is not available yet; export as PDF or TXT".to_string())
        }
        _ => fs::read_to_string(path).map_err(|e| {
            format!(
                "unsupported or unreadable syllabus file '{}': {e}",
                path.display()
            )
        }),
    }
}

fn title_from_file_name(file_name: &str) -> String {
    file_name
        .rsplit_once('.')
        .map_or(file_name, |(base, _)| base)
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
pub async fn get_syllabus_for_review(
    state: DbState<'_>,
    syllabus_id: String,
) -> Result<SyllabusReview, String> {
    state.with_conn(|conn| {
        let syllabus = conn
            .query_row(
                "SELECT id, school_id, level_id, title, version_label, source_file_attachment_id, parser_status, llm_extraction_status, coverage_notes, is_active, created_at, updated_at, archived_at FROM syllabus_documents WHERE id = ?1",
                params![syllabus_id],
                |row| {
                    Ok(SyllabusDocument {
                        id: row.get(0)?,
                        school_id: row.get(1)?,
                        level_id: row.get(2)?,
                        title: row.get(3)?,
                        version_label: row.get(4)?,
                        source_file_attachment_id: row.get(5)?,
                        parser_status: row.get(6)?,
                        llm_extraction_status: row.get(7)?,
                        coverage_notes: row.get(8)?,
                        is_active: row.get(9)?,
                        created_at: row.get(10)?,
                        updated_at: row.get(11)?,
                        archived_at: row.get(12)?,
                    })
                },
            )
            .map_err(|e| e.to_string())?;
        let units = load_units(conn, &syllabus.id)?;
        Ok(SyllabusReview { syllabus, units })
    })
}

pub fn load_units(
    conn: &rusqlite::Connection,
    syllabus_id: &str,
) -> Result<Vec<CurriculumUnit>, String> {
    let mut stmt = conn
        .prepare("SELECT id, syllabus_id, unit_code, title, description, recommended_sequence, estimated_lessons, estimated_weeks, assessment_hint, is_required, month_label, created_at, updated_at FROM curriculum_units WHERE syllabus_id = ?1 ORDER BY recommended_sequence")
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map(params![syllabus_id], |row| {
            Ok(CurriculumUnit {
                id: row.get(0)?,
                syllabus_id: row.get(1)?,
                unit_code: row.get(2)?,
                title: row.get(3)?,
                description: row.get(4)?,
                recommended_sequence: row.get(5)?,
                estimated_lessons: row.get(6)?,
                estimated_weeks: row.get(7)?,
                assessment_hint: row.get(8)?,
                is_required: row.get(9)?,
                month_label: row.get(10)?,
                schedule_slot_count: 0,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

#[tauri::command]
pub async fn update_syllabus_review(
    state: DbState<'_>,
    syllabus_id: String,
    units: Vec<ReviewedCurriculumUnit>,
    mark_reviewed: bool,
) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "DELETE FROM curriculum_units WHERE syllabus_id = ?1",
            params![syllabus_id],
        )
        .map_err(|e| e.to_string())?;
        for (index, unit) in units.into_iter().enumerate() {
            let unit_id = unit
                .id
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| new_id("unit"));
            let title = {
                let clean = unit.title.trim();
                if clean.is_empty() {
                    format!("Unit {}", index + 1)
                } else {
                    clean.to_string()
                }
            };
            let recommended_sequence = (index + 1) as i32;
            let estimated_lessons = unit.estimated_lessons.unwrap_or(4).max(1);
            let estimated_weeks = unit.estimated_weeks.unwrap_or(estimated_lessons).max(0);
            let created_at = now();
            let updated_at = now();
            conn.execute(
                "INSERT INTO curriculum_units (id, syllabus_id, unit_code, title, description, recommended_sequence, estimated_lessons, estimated_weeks, assessment_hint, is_required, month_label, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    unit_id,
                    syllabus_id,
                    unit.unit_code,
                    title,
                    unit.description,
                    recommended_sequence,
                    estimated_lessons,
                    estimated_weeks,
                    unit.assessment_hint,
                    unit.is_required.unwrap_or(true),
                    unit.month_label,
                    created_at,
                    updated_at
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        let status = if mark_reviewed { "reviewed" } else { "completed" };
        conn.execute(
            "UPDATE syllabus_documents SET llm_extraction_status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now(), syllabus_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub async fn run_advanced_curriculum_extraction(
    state: DbState<'_>,
    syllabus_id: String,
) -> Result<Vec<CurriculumUnit>, String> {
    use crate::services::llm_service::{extract_units_with_provider, load_llm_config};
    state.with_conn(|conn| {
        let text: String = conn
            .query_row(
                "SELECT COALESCE(coverage_notes, '') FROM syllabus_documents WHERE id = ?1",
                params![syllabus_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if text.trim().is_empty() {
            return Err("no extracted syllabus text is available".to_string());
        }
        let config = load_llm_config(conn);
        let units = match extract_units_with_provider(&config, &text, &syllabus_id) {
            Ok(units) => units,
            Err(err) => {
                conn.execute(
                    "UPDATE syllabus_documents SET llm_extraction_status = 'failed', updated_at = ?1 WHERE id = ?2",
                    params![crate::services::hierarchy::now(), syllabus_id],
                )
                .map_err(|e| e.to_string())?;
                return Err(err);
            }
        };
        conn.execute(
            "DELETE FROM curriculum_units WHERE syllabus_id = ?1",
            params![syllabus_id],
        )
        .map_err(|e| e.to_string())?;
        for unit in &units {
            conn.execute(
                "INSERT INTO curriculum_units (id, syllabus_id, unit_code, title, description, recommended_sequence, estimated_lessons, estimated_weeks, assessment_hint, is_required, month_label, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    unit.id,
                    unit.syllabus_id,
                    unit.unit_code,
                    unit.title,
                    unit.description,
                    unit.recommended_sequence,
                    unit.estimated_lessons,
                    unit.estimated_weeks,
                    unit.assessment_hint,
                    unit.is_required,
                    unit.month_label,
                    unit.created_at,
                    unit.updated_at
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(units)
    })
}
