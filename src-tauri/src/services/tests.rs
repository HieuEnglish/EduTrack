use crate::services::hierarchy::{get_class, get_level, new_id, now, DbState};
use crate::services::llm_service::{
    generate_local_report, load_and_probe_llm_config, probe_opencode_cli,
};
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestTemplate {
    pub id: String,
    pub class_id: String,
    pub school_id: String,
    pub level_id: String,
    pub title: String,
    pub instructions: String,
    pub scoring_rubric: String,
    pub max_score: f64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentSubmission {
    pub student_id: String,
    pub student_name: String,
    pub assessment_id: String,
    pub score_value: Option<f64>,
    pub score_label: Option<String>,
    pub teacher_comment: Option<String>,
    pub attachment_id: Option<String>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
struct SubmissionContent {
    text: String,
    source_note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GradingToolStatus {
    pub key: String,
    pub label: String,
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GradingDiagnostics {
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_available: bool,
    pub llm_detail: String,
    pub tools: Vec<GradingToolStatus>,
    pub storage_root: String,
    pub storage_pattern: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GradingToolInstallFailure {
    pub key: String,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GradingToolInstallResult {
    pub installed: Vec<String>,
    pub already_available: Vec<String>,
    pub failed: Vec<GradingToolInstallFailure>,
    pub notes: Vec<String>,
    pub post_checks: Vec<GradingToolStatus>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestDraft {
    pub title: String,
    pub instructions: String,
    pub scoring_rubric: String,
    pub max_score: f64,
}

#[derive(Debug, Clone)]
struct SyllabusUnitDraft {
    unit_code: Option<String>,
    title: String,
    description: Option<String>,
    assessment_hint: Option<String>,
    estimated_lessons: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestSyllabusSection {
    pub id: String,
    pub unit_code: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub estimated_lessons: i32,
    pub assessment_hint: Option<String>,
    pub is_required: bool,
}

#[tauri::command]
pub async fn create_test(
    state: DbState<'_>,
    class_id: String,
    title: String,
    instructions: String,
    scoring_rubric: String,
    max_score: f64,
) -> Result<String, String> {
    state.with_conn(|conn| {
        let _class = get_class(conn, &class_id)?;
        create_test_rows(conn, &class_id, &title, &instructions, &scoring_rubric, max_score)
    })
}

#[tauri::command]
pub async fn create_test_based_on_syllabus(
    state: DbState<'_>,
    class_id: String,
    selected_sections: Vec<TestSyllabusSection>,
) -> Result<TestDraft, String> {
    state.with_conn(|conn| {
        let class = get_class(conn, &class_id)?;
        let level = get_level(conn, &class.level_id)?;
        let syllabus_id = match level.default_syllabus_id.clone() {
            Some(id) if !id.trim().is_empty() => Some(id),
            _ => conn
                .query_row(
                    "SELECT id
                     FROM syllabus_documents
                     WHERE level_id = ?1 AND archived_at IS NULL
                     ORDER BY is_active DESC, updated_at DESC
                     LIMIT 1",
                    params![class.level_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|e| e.to_string())?,
        };

        let syllabus_id = syllabus_id.ok_or_else(|| {
            "No syllabus found for this class level. Upload and review a syllabus first.".to_string()
        })?;

        let syllabus_title = conn
            .query_row(
                "SELECT COALESCE(title, '') FROM syllabus_documents WHERE id = ?1",
                params![syllabus_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())?;

        let units: Vec<SyllabusUnitDraft> = selected_sections
            .into_iter()
            .filter(|section| section.is_required)
            .map(|section| SyllabusUnitDraft {
                unit_code: section.unit_code,
                title: section.title,
                description: section.description,
                assessment_hint: section.assessment_hint,
                estimated_lessons: section.estimated_lessons.max(1),
            })
            .collect();
        if units.is_empty() {
            return Err(
                "No syllabus sections selected. Select at least one section to build the test."
                    .to_string(),
            );
        }

        let subject = class
            .subject_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Class");
        let focus_units: Vec<&SyllabusUnitDraft> = units.iter().take(4).collect();
        let focus_count = focus_units.len().max(1);
        let per_unit_points = (80_i32 / focus_count as i32).max(10);
        let unit_total = per_unit_points * focus_count as i32;
        let synthesis_points = 100 - unit_total;
        let total_lessons: i32 = focus_units
            .iter()
            .map(|u| if u.estimated_lessons > 0 { u.estimated_lessons } else { 2 })
            .sum();

        let mut instructions = Vec::<String>::new();
        instructions.push(format!(
            "Syllabus-based assessment for {subject} ({}) aligned to: {}.",
            class.name, syllabus_title
        ));
        instructions.push(format!(
            "Covers {} syllabus unit(s) and approximately {} lesson(s).",
            focus_units.len(),
            total_lessons
        ));
        instructions.push(String::new());
        instructions.push("Student directions:".to_string());
        instructions.push("1. Answer each section using precise vocabulary from the unit.".to_string());
        instructions.push(
            "2. Show reasoning and evidence (examples, worked steps, or textual references)."
                .to_string(),
        );
        instructions.push("3. Write clearly enough for another student to follow your thinking.".to_string());
        instructions.push(String::new());
        instructions.push("Assessment sections:".to_string());

        for (index, unit) in focus_units.iter().enumerate() {
            let unit_label = match unit.unit_code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(code) => format!("{code} - {}", unit.title),
                None => unit.title.clone(),
            };
            let concept = concise_unit_focus(unit.description.as_deref(), &unit.title);
            let assessment_hint = unit
                .assessment_hint
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("a short written response");
            instructions.push(format!(
                "Section {} ({} pts): {}",
                index + 1,
                per_unit_points,
                unit_label
            ));
            instructions.push(format!(
                "- Knowledge: Explain {} with one accurate example.",
                concept
            ));
            instructions.push(format!(
                "- Application: Complete {} that demonstrates {}.",
                assessment_hint, concept
            ));
        }
        instructions.push(String::new());
        instructions.push(format!(
            "Final synthesis ({} pts): connect at least two units and justify your conclusion with evidence.",
            synthesis_points.max(10)
        ));

        let rubric = format!(
            "Total: 100 points\n\
             - Accuracy and unit vocabulary: 40 points\n\
             - Evidence and reasoning quality: 35 points\n\
             - Application and transfer across contexts: 25 points\n\
             \n\
             Marking guidance:\n\
             - Full marks: precise terminology, complete reasoning, correct evidence.\n\
             - Mid-range: mostly correct ideas but missing depth or evidence.\n\
             - Low: major misconceptions or unsupported answers.\n\
             \n\
             Use syllabus unit goals from '{}' as the scoring anchor.",
            syllabus_title
        );

        Ok(TestDraft {
            title: format!("{subject} Syllabus Assessment"),
            instructions: instructions.join("\n"),
            scoring_rubric: rubric,
            max_score: 100.0,
        })
    })
}

#[tauri::command]
pub async fn get_test_submissions(
    state: DbState<'_>,
    assessment_id: String,
) -> Result<Vec<StudentSubmission>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT s.id, s.full_name, as2.id, as2.score_value, as2.score_label, as2.teacher_comment,
                    a.id as att_id, a.file_name, a.mime_type,
                    CASE WHEN as2.score_value IS NOT NULL THEN 'scored' ELSE 'pending' END as status
             FROM assessment_scores as2
             JOIN students s ON as2.student_id = s.id AND s.archived_at IS NULL
             LEFT JOIN attachments a ON a.owner_type = 'assessment_score' AND a.owner_id = as2.id
             WHERE as2.assessment_id = ?1
             ORDER BY s.full_name"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![assessment_id], |row| {
            Ok(StudentSubmission {
                student_id: row.get(0)?,
                student_name: row.get(1)?,
                assessment_id: row.get(2)?,
                score_value: row.get(3)?,
                score_label: row.get(4)?,
                teacher_comment: row.get(5)?,
                attachment_id: row.get(6)?,
                file_name: row.get(7)?,
                mime_type: row.get(8)?,
                status: row.get(9)?,
            })
        }).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub async fn upload_submission_file(
    state: DbState<'_>,
    assessment_id: String,
    student_id: String,
    file_name: String,
    file_data: Vec<u8>,
    mime_type: String,
) -> Result<String, String> {
    let (score_id, class_id, level_id, school_id) = state.with_conn(|conn| {
        conn.query_row(
            "SELECT as2.id, a.class_id, c.level_id, c.school_id
             FROM assessment_scores as2
             JOIN assessments a ON a.id = as2.assessment_id
             JOIN classes c ON c.id = a.class_id
             WHERE as2.assessment_id = ?1 AND as2.student_id = ?2",
            params![&assessment_id, &student_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .map_err(|e| e.to_string())
    })?;

    let structured_dir = state
        .uploads_dir("submissions")
        .join(format!("school_{}", sanitize_path_component(&school_id)))
        .join(format!("level_{}", sanitize_path_component(&level_id)))
        .join(format!("class_{}", sanitize_path_component(&class_id)))
        .join(format!("assessment_{}", sanitize_path_component(&assessment_id)))
        .join(format!("student_{}", sanitize_path_component(&student_id)));
    let upload_dir = structured_dir;
    std::fs::create_dir_all(&upload_dir).map_err(|e| e.to_string())?;
    let safe_name = sanitize_filename(&file_name);
    let stamped_name = format!("{}_{}", upload_stamp(), safe_name);
    let file_path = upload_dir.join(&stamped_name);
    std::fs::write(&file_path, &file_data).map_err(|e| e.to_string())?;

    state.with_conn(|conn| {
        let att_id = new_id("attachment");
        conn.execute(
            "INSERT INTO attachments (id, owner_type, owner_id, file_name, mime_type, size_bytes, storage_path, sha256, created_at)
             VALUES (?1, 'assessment_score', ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
            params![att_id, score_id, stamped_name, mime_type, file_data.len() as i64, file_path.to_string_lossy().to_string(), now()],
        ).map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE assessment_scores SET teacher_comment = COALESCE(teacher_comment, '') || '\n[File uploaded: ' || ?1 || ']', updated_at = ?2 WHERE id = ?3",
            params![stamped_name, now(), score_id],
        ).map_err(|e| e.to_string())?;

        Ok(att_id)
    })
}

#[tauri::command]
pub async fn upload_submission_link(
    state: DbState<'_>,
    assessment_id: String,
    student_id: String,
    file_url: String,
) -> Result<String, String> {
    let url = file_url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("Only http/https links are supported".to_string());
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(url).send().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }

    let mime_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let mut file_name = url
        .split('/')
        .next_back()
        .map(|part| part.split('?').next().unwrap_or(part))
        .filter(|part| !part.trim().is_empty())
        .unwrap_or("submission.bin")
        .to_string();
    if file_name.len() > 160 {
        file_name.truncate(160);
    }

    let file_data = response.bytes().map_err(|e| e.to_string())?.to_vec();

    upload_submission_file(
        state,
        assessment_id,
        student_id,
        file_name,
        file_data,
        mime_type,
    )
    .await
}

#[tauri::command]
pub async fn get_grading_diagnostics(state: DbState<'_>) -> Result<GradingDiagnostics, String> {
    let llm = state.with_conn(|conn| Ok(load_and_probe_llm_config(conn)))?;
    let tools = vec![
        probe_tool("opencode", "OpenCode CLI", &["--version"]),
        probe_tool("tesseract", "Tesseract OCR", &["--version"]),
        probe_tool("whisper", "Whisper CLI", &["--help"]),
        probe_tool("pandoc", "Pandoc", &["--version"]),
        probe_tool("antiword", "Antiword (.doc)", &["-h"]),
        probe_tool("catdoc", "Catdoc (.doc)", &["-h"]),
    ];

    Ok(GradingDiagnostics {
        llm_provider: llm.provider,
        llm_model: llm.model,
        llm_available: llm.available,
        llm_detail: llm.detail,
        tools,
        storage_root: state.uploads_dir("submissions").to_string_lossy().to_string(),
        storage_pattern:
            "submissions/school_<schoolId>/level_<levelId>/class_<classId>/assessment_<assessmentId>/student_<studentId>/timestamp_filename"
                .to_string(),
    })
}

#[tauri::command]
pub async fn install_grading_tools(
    tool_keys: Vec<String>,
) -> Result<GradingToolInstallResult, String> {
    let requested = normalize_install_targets(tool_keys);
    if requested.is_empty() {
        return Err("No supported tool keys requested. Supported keys: opencode, tesseract, whisper, pandoc".to_string());
    }

    Ok(run_grading_tool_install_pass(&requested))
}

fn run_grading_tool_install_pass(requested: &[String]) -> GradingToolInstallResult {
    let requested_vec = requested.to_vec();

    let mut installed = Vec::new();
    let mut already_available = Vec::new();
    let mut failed = Vec::new();
    let mut notes = Vec::new();

    for key in &requested_vec {
        let label = tool_label(key);
        let probe_before = probe_tool(key, label, tool_probe_args(key));
        if probe_before.available {
            already_available.push(key.clone());
            continue;
        }

        match install_tool_for_key(key) {
            Ok(mut install_notes) => notes.append(&mut install_notes),
            Err(err) => notes.push(format!("{key}: {err}")),
        }

        let probe_after = probe_tool(key, label, tool_probe_args(key));
        if probe_after.available {
            installed.push(key.clone());
        } else {
            failed.push(GradingToolInstallFailure {
                key: key.clone(),
                label: label.to_string(),
                detail: probe_after.detail,
            });
        }
    }

    let post_checks = requested_vec
        .iter()
        .map(|key| probe_tool(key, tool_label(key), tool_probe_args(key)))
        .collect();

    GradingToolInstallResult {
        installed,
        already_available,
        failed,
        notes,
        post_checks,
    }
}

#[tauri::command]
pub async fn score_submission(
    state: DbState<'_>,
    assessment_id: String,
    student_id: String,
    score: f64,
    label: Option<String>,
    comment: Option<String>,
) -> Result<(), String> {
    state.with_conn(|conn| {
        update_submission_score(
            conn,
            &assessment_id,
            &student_id,
            score,
            label.as_deref(),
            comment.as_deref(),
        )
    })
}

fn create_test_rows(
    conn: &Connection,
    class_id: &str,
    title: &str,
    instructions: &str,
    scoring_rubric: &str,
    max_score: f64,
) -> Result<String, String> {
    let id = new_id("test");
    let timestamp = now();
    conn.execute(
        "INSERT INTO assessments (id, class_id, title, assessment_date, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            class_id,
            title,
            timestamp.clone(),
            format!("INSTRUCTIONS:{instructions}\nRUBRIC:{scoring_rubric}\nMAX_SCORE:{max_score}"),
            timestamp,
            timestamp
        ],
    )
    .map_err(|e| e.to_string())?;

    let students: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT id FROM students WHERE class_id = ?1 AND archived_at IS NULL")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![class_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    for sid in &students {
        let score_id = new_id("score");
        conn.execute(
            "INSERT INTO assessment_scores (id, assessment_id, student_id, score_value, score_label, teacher_comment, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, NULL, NULL, ?4, ?4)",
            params![score_id, id, sid, timestamp],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(id)
}

fn update_submission_score(
    conn: &Connection,
    assessment_id: &str,
    student_id: &str,
    score: f64,
    label: Option<&str>,
    comment: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE assessment_scores SET score_value = ?1, score_label = ?2, teacher_comment = ?3, updated_at = ?4
         WHERE assessment_id = ?5 AND student_id = ?6",
        params![score, label, comment, now(), assessment_id, student_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn auto_score_submissions(
    state: DbState<'_>,
    assessment_id: String,
) -> Result<Vec<StudentSubmission>, String> {
    let (config_provider, config_model, llm_available) = state.with_conn(|conn| {
        let config = load_and_probe_llm_config(conn);
        Ok((config.provider, config.model, config.available))
    })?;

    state.with_conn(|conn| {
        let rubric: String = conn.query_row(
            "SELECT COALESCE(description, '') FROM assessments WHERE id = ?1",
            params![assessment_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        let (instructions, scoring_rubric, max_score) = parse_assessment_payload(&rubric);

        let mut stmt = conn.prepare(
            "SELECT as2.id, as2.student_id, s.full_name, a.file_name, a.mime_type, a.storage_path
             FROM assessment_scores as2
             JOIN students s ON as2.student_id = s.id
             LEFT JOIN attachments a ON a.owner_type = 'assessment_score' AND a.owner_id = as2.id
             WHERE as2.assessment_id = ?1 AND s.archived_at IS NULL"
        ).map_err(|e| e.to_string())?;

        let llm_config = crate::services::llm_service::LocalModelConfig {
            provider: config_provider,
            model: config_model,
            available: llm_available,
            detail: String::new(),
        };

        let mut results = Vec::new();
        let rows = stmt.query_map(params![assessment_id], |row| {
            let score_id: String = row.get(0)?;
            let student_id: String = row.get(1)?;
            let student_name: String = row.get(2)?;
            let file_name: Option<String> = row.get(3)?;
            let mime_type: Option<String> = row.get(4)?;
            let storage_path: Option<String> = row.get(5)?;

            let fname = file_name.clone().unwrap_or_default();

            let (auto_score, label, comment) = if let Some(path) = storage_path {
                match extract_submission_content_for_scoring(&path, mime_type.as_deref()) {
                    Ok(content) if llm_available && !content.text.trim().is_empty() => {
                        let prompt = format!(
                            "You are an expert teacher and grader.\n\n\
                             Assignment instructions:\n{instructions}\n\n\
                             Scoring rubric:\n{scoring_rubric}\n\n\
                             Maximum score: {max_score}\n\n\
                             Student: {student_name}\n\
                             Submission source: {source}\n\
                             Submission content:\n---\n{submission}\n---\n\n\
                             Return ONLY valid JSON with this shape:\n\
                             {{\n\
                               \"score\": number,\n\
                               \"label\": string,\n\
                               \"summary\": string,\n\
                               \"whatWentWrong\": string[],\n\
                               \"improvementSteps\": string[]\n\
                             }}\n\
                             Rules:\n\
                             - score must be between 0 and {max_score}\n\
                             - label should be one of: Excellent, Good, Developing, Needs Improvement\n\
                             - summary must be specific and concise\n\
                             - include at least 2 items in whatWentWrong and improvementSteps when possible.",
                            source = content.source_note,
                            submission = content.text.chars().take(22_000).collect::<String>(),
                        );
                        match generate_local_report(&llm_config, &prompt) {
                            Ok(response) => {
                                if let Some((parsed_score, parsed_label, parsed_feedback)) =
                                    parse_grading_response(&response, max_score)
                                {
                                    (parsed_score, parsed_label, parsed_feedback)
                                } else {
                                    let fallback = (max_score * 0.75).round().max(1.0);
                                    (
                                        fallback,
                                        "Auto-assessed".to_string(),
                                        format!(
                                            "{}\n\nModel response could not be fully parsed. Review and adjust.\nRaw model output:\n{}",
                                            content.source_note,
                                            response.trim().chars().take(1200).collect::<String>()
                                        ),
                                    )
                                }
                            }
                            Err(err) => {
                                let fallback = (max_score * 0.75).round().max(1.0);
                                (
                                    fallback,
                                    "Auto-assessed".to_string(),
                                    format!(
                                        "{}\n\nAuto-scored estimate used because LLM was unavailable ({err}). Review and adjust.",
                                        content.source_note
                                    ),
                                )
                            }
                        }
                    }
                    Ok(content) => {
                        let fallback = (max_score * 0.7).round().max(1.0);
                        (
                            fallback,
                            "Needs review".to_string(),
                            format!(
                                "{}\n\nContent was extracted, but LLM auto-grading is unavailable. Estimated score set for teacher review.",
                                content.source_note
                            ),
                        )
                    }
                    Err(err) => (
                        0.0,
                        "Needs review".to_string(),
                        format!(
                            "File '{fname}' could not be auto-processed.\nReason: {err}\nPlease review manually or upload a text/PDF/docx submission."
                        ),
                    ),
                }
            } else {
                (0.0, "No submission".to_string(),
                 "No file submitted. Score set to 0.".to_string())
            };

            conn.execute(
                "UPDATE assessment_scores SET score_value = ?1, score_label = ?2, teacher_comment = ?3, updated_at = ?4 WHERE id = ?5",
                params![auto_score, label, comment, now(), score_id],
            ).ok();

            Ok(StudentSubmission {
                student_id,
                student_name,
                assessment_id: assessment_id.clone(),
                score_value: Some(auto_score),
                score_label: Some(label),
                teacher_comment: Some(comment),
                attachment_id: None,
                file_name,
                mime_type,
                status: "scored".to_string(),
            })
        }).map_err(|e| e.to_string())?;

        for row in rows {
            results.push(row.map_err(|e| e.to_string())?);
        }
        Ok(results)
    })
}

#[tauri::command]
pub async fn auto_score_single_submission(
    state: DbState<'_>,
    assessment_id: String,
    student_id: String,
) -> Result<StudentSubmission, String> {
    let (config_provider, config_model, llm_available) = state.with_conn(|conn| {
        let config = load_and_probe_llm_config(conn);
        Ok((config.provider, config.model, config.available))
    })?;

    state.with_conn(|conn| {
        let rubric: String = conn
            .query_row(
                "SELECT COALESCE(description, '') FROM assessments WHERE id = ?1",
                params![assessment_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        let (instructions, scoring_rubric, max_score) = parse_assessment_payload(&rubric);

        let (score_id, student_name, file_name, mime_type, storage_path): (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT as2.id, s.full_name, a.file_name, a.mime_type, a.storage_path
                 FROM assessment_scores as2
                 JOIN students s ON as2.student_id = s.id
                 LEFT JOIN attachments a ON a.owner_type = 'assessment_score' AND a.owner_id = as2.id
                 WHERE as2.assessment_id = ?1 AND as2.student_id = ?2 AND s.archived_at IS NULL",
                params![assessment_id, student_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .map_err(|_| "Submission not found for student".to_string())?;

        let llm_config = crate::services::llm_service::LocalModelConfig {
            provider: config_provider,
            model: config_model,
            available: llm_available,
            detail: String::new(),
        };

        let fname = file_name.clone().unwrap_or_default();
        let (auto_score, label, comment) = if let Some(path) = storage_path {
            match extract_submission_content_for_scoring(&path, mime_type.as_deref()) {
                Ok(content) if llm_available && !content.text.trim().is_empty() => {
                    let prompt = format!(
                        "You are an expert teacher and grader.\n\n\
                         Assignment instructions:\n{instructions}\n\n\
                         Scoring rubric:\n{scoring_rubric}\n\n\
                         Maximum score: {max_score}\n\n\
                         Student: {student_name}\n\
                         Submission source: {source}\n\
                         Submission content:\n---\n{submission}\n---\n\n\
                         Return ONLY valid JSON with this shape:\n\
                         {{\n\
                           \"score\": number,\n\
                           \"label\": string,\n\
                           \"summary\": string,\n\
                           \"whatWentWrong\": string[],\n\
                           \"improvementSteps\": string[]\n\
                         }}\n\
                         Rules:\n\
                         - score must be between 0 and {max_score}\n\
                         - label should be one of: Excellent, Good, Developing, Needs Improvement\n\
                         - summary must be specific and concise\n\
                         - include at least 2 items in whatWentWrong and improvementSteps when possible.",
                        source = content.source_note,
                        submission = content.text.chars().take(22_000).collect::<String>(),
                    );
                    match generate_local_report(&llm_config, &prompt) {
                        Ok(response) => {
                            if let Some((parsed_score, parsed_label, parsed_feedback)) =
                                parse_grading_response(&response, max_score)
                            {
                                (parsed_score, parsed_label, parsed_feedback)
                            } else {
                                let fallback = (max_score * 0.75).round().max(1.0);
                                (
                                    fallback,
                                    "Auto-assessed".to_string(),
                                    format!(
                                        "{}\n\nModel response could not be fully parsed. Review and adjust.\nRaw model output:\n{}",
                                        content.source_note,
                                        response.trim().chars().take(1200).collect::<String>()
                                    ),
                                )
                            }
                        }
                        Err(err) => {
                            let fallback = (max_score * 0.75).round().max(1.0);
                            (
                                fallback,
                                "Auto-assessed".to_string(),
                                format!(
                                    "{}\n\nAuto-scored estimate used because LLM was unavailable ({err}). Review and adjust.",
                                    content.source_note
                                ),
                            )
                        }
                    }
                }
                Ok(content) => {
                    let fallback = (max_score * 0.7).round().max(1.0);
                    (
                        fallback,
                        "Needs review".to_string(),
                        format!(
                            "{}\n\nContent was extracted, but LLM auto-grading is unavailable. Estimated score set for teacher review.",
                            content.source_note
                        ),
                    )
                }
                Err(err) => (
                    0.0,
                    "Needs review".to_string(),
                    format!(
                        "File '{fname}' could not be auto-processed.\nReason: {err}\nPlease review manually or upload a text/PDF/docx submission."
                    ),
                ),
            }
        } else {
            (
                0.0,
                "No submission".to_string(),
                "No file submitted. Score set to 0.".to_string(),
            )
        };

        conn.execute(
            "UPDATE assessment_scores SET score_value = ?1, score_label = ?2, teacher_comment = ?3, updated_at = ?4 WHERE id = ?5",
            params![auto_score, label, comment, now(), score_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(StudentSubmission {
            student_id,
            student_name,
            assessment_id,
            score_value: Some(auto_score),
            score_label: Some(label),
            teacher_comment: Some(comment),
            attachment_id: None,
            file_name,
            mime_type,
            status: "scored".to_string(),
        })
    })
}

#[tauri::command]
pub async fn get_class_tests(
    state: DbState<'_>,
    class_id: String,
) -> Result<Vec<TestTemplate>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT a.id, a.class_id, c.school_id, c.level_id, a.title,
                    COALESCE(a.description, '') as instructions,
                    '' as scoring_rubric,
                    100.0 as max_score,
                    CASE WHEN COUNT(as2.id) > 0 AND SUM(CASE WHEN as2.score_value IS NOT NULL THEN 1 ELSE 0 END) = COUNT(as2.id) THEN 'completed' ELSE 'pending' END as status,
                    a.created_at, a.updated_at
             FROM assessments a
             JOIN classes c ON a.class_id = c.id
             LEFT JOIN assessment_scores as2 ON as2.assessment_id = a.id
             WHERE a.class_id = ?1
             GROUP BY a.id
             ORDER BY a.created_at DESC"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![class_id], |row| {
            let payload: String = row.get(5)?;
            let (instructions, rubric_str, max) = parse_assessment_payload(&payload);
            Ok(TestTemplate {
                id: row.get(0)?,
                class_id: row.get(1)?,
                school_id: row.get(2)?,
                level_id: row.get(3)?,
                title: row.get(4)?,
                instructions,
                scoring_rubric: rubric_str,
                max_score: max,
                status: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        }).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

fn concise_unit_focus(description: Option<&str>, fallback: &str) -> String {
    let Some(raw) = description.map(str::trim).filter(|s| !s.is_empty()) else {
        return fallback.trim().to_string();
    };
    let mut sentence = raw
        .split(['.', '!', '?'])
        .map(str::trim)
        .find(|segment| !segment.is_empty())
        .unwrap_or(raw)
        .to_string();
    if sentence.len() > 100 {
        sentence.truncate(100);
        sentence.push_str("...");
    }
    sentence
}

fn parse_assessment_payload(payload: &str) -> (String, String, f64) {
    let trimmed = payload.trim();
    if !trimmed.contains("INSTRUCTIONS:") && !trimmed.contains("RUBRIC:") && !trimmed.contains("MAX_SCORE:") {
        return (trimmed.to_string(), String::new(), 100.0);
    }
    let instructions = trimmed
        .split("RUBRIC:")
        .next()
        .unwrap_or(trimmed)
        .replace("INSTRUCTIONS:", "")
        .trim()
        .to_string();
    let scoring_rubric = trimmed
        .split("RUBRIC:")
        .nth(1)
        .and_then(|s| s.split("MAX_SCORE:").next())
        .unwrap_or("")
        .trim()
        .to_string();
    let max_score = trimmed
        .split("MAX_SCORE:")
        .nth(1)
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(100.0);
    (instructions, scoring_rubric, max_score)
}

fn extract_submission_content_for_scoring(
    path: &str,
    mime_type: Option<&str>,
) -> Result<SubmissionContent, String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    let mime = mime_type.unwrap_or_default().to_ascii_lowercase();

    let text = if extension == "pdf" || mime == "application/pdf" {
        pdf_extract::extract_text(path).map_err(|e| format!("failed to extract PDF text: {e}"))?
    } else if is_image_submission(&extension, &mime) {
        extract_image_text_with_tesseract_cli(path)?
    } else if extension == "docx"
        || extension == "doc"
        || mime
            .contains("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    {
        extract_word_document_text(path, &extension)?
    } else if is_textual_submission(&extension, &mime) {
        fs::read_to_string(path).or_else(|_| {
            let bytes = fs::read(path)?;
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }).map_err(|e: std::io::Error| e.to_string())?
    } else if is_audio_submission(&extension, &mime) {
        transcribe_audio_with_whisper_cli(path)?
    } else {
        return Err(format!(
            "unsupported file type '{}' (mime: {}).",
            extension,
            mime_type.unwrap_or("unknown")
        ));
    };

    let normalized = text
        .replace('\u{0000}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return Err("extracted content was empty".to_string());
    }

    let source_note = if extension == "pdf" {
        "Source: PDF text extraction.".to_string()
    } else if is_image_submission(&extension, &mime) {
        "Source: OCR from image submission via local Tesseract CLI.".to_string()
    } else if extension == "docx" || extension == "doc" {
        "Source: Word document text extraction.".to_string()
    } else if is_audio_submission(&extension, &mime) {
        "Source: Audio transcription via local Whisper CLI.".to_string()
    } else {
        "Source: Text submission.".to_string()
    };

    Ok(SubmissionContent {
        text: normalized,
        source_note,
    })
}

fn extract_image_text_with_tesseract_cli(path: &str) -> Result<String, String> {
    let out_dir: PathBuf = std::env::temp_dir().join(format!("edutrack-ocr-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let output_base = out_dir.join("ocr_output");

    let (mut tesseract_cmd, source_label) = command_for_tool("tesseract");
    let output = tesseract_cmd
        .arg(path)
        .arg(output_base.to_string_lossy().to_string())
        .arg("-l")
        .arg("eng")
        .output()
        .map_err(|e| format!("image OCR requires Tesseract ({source_label}): {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = fs::remove_dir_all(&out_dir);
        return Err(format!("image OCR failed via Tesseract: {}", stderr.trim()));
    }
    let text_path = out_dir.join("ocr_output.txt");
    let text = fs::read_to_string(&text_path)
        .map_err(|e| format!("failed to read OCR output: {e}"))?;
    let _ = fs::remove_dir_all(&out_dir);
    if text.trim().is_empty() {
        return Err("image OCR returned empty text".to_string());
    }
    Ok(text)
}

fn extract_word_document_text(path: &str, extension: &str) -> Result<String, String> {
    if let Ok(text) = extract_with_pandoc(path) {
        if !text.trim().is_empty() {
            return Ok(text);
        }
    }

    if extension == "docx" {
        if let Ok(text) = extract_docx_with_powershell(path) {
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    if extension == "doc" {
        for cmd in ["antiword", "catdoc"] {
            if let Ok(text) = extract_with_simple_cli(cmd, path) {
                if !text.trim().is_empty() {
                    return Ok(text);
                }
            }
        }
    }

    Err(
        "Unable to read DOC/DOCX automatically. Install one of: pandoc (recommended), antiword/catdoc (.doc), or upload as PDF/TXT."
            .to_string(),
    )
}

fn extract_with_pandoc(path: &str) -> Result<String, String> {
    let (mut pandoc_cmd, source_label) = command_for_tool("pandoc");
    let output = pandoc_cmd
        .arg(path)
        .arg("-t")
        .arg("plain")
        .output()
        .map_err(|e| format!("pandoc not available ({source_label}): {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn extract_docx_with_powershell(path: &str) -> Result<String, String> {
    let script = r#"
$InputPath = $args[0]
$tmp = Join-Path $env:TEMP ("edutrack-docx-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  [System.IO.Compression.ZipFile]::ExtractToDirectory($InputPath, $tmp)
  $xmlPath = Join-Path $tmp "word\document.xml"
  if (!(Test-Path -LiteralPath $xmlPath)) { exit 3 }
  $xml = Get-Content -Raw -LiteralPath $xmlPath
  $xml = $xml -replace "</w:p>", "`n"
  $text = [System.Text.RegularExpressions.Regex]::Replace($xml, "<[^>]+>", " ")
  $text = $text -replace "&amp;", "&"
  $text = $text -replace "&lt;", "<"
  $text = $text -replace "&gt;", ">"
  $text = $text -replace "&quot;", '"'
  $text = $text -replace "&apos;", "'"
  Write-Output $text
} finally {
  Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
"#;
    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .arg(path)
        .output()
        .map_err(|e| format!("powershell extraction failed: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        return Err("empty DOCX extraction output".to_string());
    }
    Ok(text)
}

fn extract_with_simple_cli(command: &str, path: &str) -> Result<String, String> {
    let output = Command::new(command)
        .arg(path)
        .output()
        .map_err(|e| format!("{command} not available: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn transcribe_audio_with_whisper_cli(path: &str) -> Result<String, String> {
    let input_path = Path::new(path);
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("submission");
    let out_dir: PathBuf = std::env::temp_dir().join(format!("edutrack-whisper-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let whisper_output = run_whisper_transcription(path, &out_dir).map_err(|err| {
        let _ = fs::remove_dir_all(&out_dir);
        err
    })?;
    if !whisper_output.status.success() {
        let stderr = String::from_utf8_lossy(&whisper_output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&whisper_output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        let _ = fs::remove_dir_all(&out_dir);
        return Err(format!(
            "audio transcription failed via Whisper: {}",
            if detail.is_empty() {
                format!("process exited with {}", whisper_output.status)
            } else {
                detail
            }
        ));
    }

    let transcript_path = out_dir.join(format!("{stem}.txt"));
    let transcript = fs::read_to_string(&transcript_path)
        .map_err(|e| format!("failed to read Whisper transcript: {e}"))?;
    let _ = fs::remove_dir_all(&out_dir);
    if transcript.trim().is_empty() {
        return Err("audio transcript was empty".to_string());
    }
    Ok(transcript)
}

fn parse_grading_response(raw: &str, max_score: f64) -> Option<(f64, String, String)> {
    let json_candidate = raw
        .find('{')
        .and_then(|start| raw.rfind('}').map(|end| (start, end)))
        .and_then(|(start, end)| raw.get(start..=end))
        .unwrap_or(raw)
        .trim();
    let parsed: serde_json::Value = serde_json::from_str(json_candidate).ok()?;
    let score = parsed
        .get("score")
        .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse::<f64>().ok()))?;
    let clamped = score.clamp(0.0, max_score);
    let rounded = (clamped * 2.0).round() / 2.0;
    let label = parsed
        .get("label")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Auto-scored")
        .to_string();
    let summary = parsed
        .get("summary")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("No summary provided.");
    let wrong = json_string_array(parsed.get("whatWentWrong"))
        .into_iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>();
    let improve = json_string_array(parsed.get("improvementSteps"))
        .into_iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>();
    let feedback = format!(
        "Summary: {summary}\n\nWhat was wrong:\n{}\n\nHow to improve:\n{}",
        if wrong.is_empty() { "- Not specified".to_string() } else { wrong.join("\n") },
        if improve.is_empty() { "- Not specified".to_string() } else { improve.join("\n") },
    );
    Some((rounded, label, feedback))
}

fn json_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn is_textual_submission(extension: &str, mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/yaml"
                | "text/markdown"
        )
        || matches!(
            extension,
            "txt"
                | "md"
                | "csv"
                | "tsv"
                | "json"
                | "xml"
                | "html"
                | "htm"
                | "yaml"
                | "yml"
                | "log"
                | "ini"
                | "toml"
                | "rtf"
        )
}

fn is_audio_submission(extension: &str, mime: &str) -> bool {
    mime.starts_with("audio/")
        || matches!(
            extension,
            "mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" | "oga" | "opus" | "webm" | "wma"
        )
}

fn is_image_submission(extension: &str, mime: &str) -> bool {
    mime.starts_with("image/")
        || matches!(
            extension,
            "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff" | "gif" | "heic"
        )
}

fn sanitize_path_component(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    }
}

fn sanitize_filename(value: &str) -> String {
    let trimmed = value.trim();
    let safe: String = trimmed
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect();
    if safe.is_empty() {
        "submission.bin".to_string()
    } else {
        safe
    }
}

fn upload_stamp() -> String {
    now()
        .replace([':', '.'], "")
        .replace('-', "")
        .replace('T', "_")
        .replace('Z', "")
}

fn normalize_install_targets(requested: Vec<String>) -> Vec<String> {
    let mut keys: Vec<String> = if requested.is_empty() {
        vec![
            "opencode".to_string(),
            "tesseract".to_string(),
            "whisper".to_string(),
            "pandoc".to_string(),
        ]
    } else {
        requested
            .into_iter()
            .map(|key| key.trim().to_ascii_lowercase())
            .collect()
    };
    keys.retain(|key| matches!(key.as_str(), "opencode" | "tesseract" | "whisper" | "pandoc"));
    keys.sort();
    keys.dedup();
    keys
}

fn tool_label(key: &str) -> &'static str {
    match key {
        "opencode" => "OpenCode CLI",
        "tesseract" => "Tesseract OCR",
        "whisper" => "Whisper CLI",
        "pandoc" => "Pandoc",
        _ => "Tool",
    }
}

fn tool_probe_args(key: &str) -> &'static [&'static str] {
    match key {
        "whisper" => &["--help"],
        _ => &["--version"],
    }
}

fn install_tool_for_key(key: &str) -> Result<Vec<String>, String> {
    #[cfg(windows)]
    {
        match key {
            "opencode" => install_opencode_windows(),
            "tesseract" => install_tesseract_windows(),
            "pandoc" => install_pandoc_windows(),
            "whisper" => install_whisper_windows(),
            _ => Err("unsupported tool key".to_string()),
        }
    }

    #[cfg(target_os = "macos")]
    {
        match key {
            "opencode" => install_opencode_macos(),
            "tesseract" => install_tesseract_macos(),
            "pandoc" => install_pandoc_macos(),
            "whisper" => install_whisper_unix(),
            _ => Err("unsupported tool key".to_string()),
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        match key {
            "opencode" => install_opencode_linux(),
            "tesseract" => install_tesseract_linux(),
            "pandoc" => install_pandoc_linux(),
            "whisper" => install_whisper_unix(),
            _ => Err("unsupported tool key".to_string()),
        }
    }
}

#[cfg(windows)]
fn install_opencode_windows() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_winget_install("SST.opencode", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_winget_install("OpenCode.OpenCode", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_choco_install("opencode", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_npm_global_install("opencode-ai", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("OpenCode install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(windows)]
fn install_tesseract_windows() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_winget_install("UB-Mannheim.TesseractOCR", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_winget_install("Tesseract-OCR.Tesseract", &mut notes).is_ok() {
        return Ok(notes);
    }
    if !is_windows_elevated() {
        notes.push("Current process is not elevated; skipping Chocolatey machine install.".to_string());
        return Err(format!(
            "Tesseract install needs Administrator privileges when winget is unavailable. {} Attempts: {}",
            windows_manual_tool_hint("tesseract"),
            notes.join(" | ")
        ));
    }
    if try_choco_install("tesseract", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("Tesseract install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(windows)]
fn install_pandoc_windows() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_winget_install("JohnMacFarlane.Pandoc", &mut notes).is_ok() {
        return Ok(notes);
    }
    if !is_windows_elevated() {
        notes.push("Current process is not elevated; skipping Chocolatey machine install.".to_string());
        return Err(format!(
            "Pandoc install needs Administrator privileges when winget is unavailable. {} Attempts: {}",
            windows_manual_tool_hint("pandoc"),
            notes.join(" | ")
        ));
    }
    if try_choco_install("pandoc", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("Pandoc install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(windows)]
fn install_whisper_windows() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    let _ = try_winget_install("Gyan.FFmpeg", &mut notes);
    let pip_targets = [
        ("python", vec!["-m", "pip", "install", "-U", "openai-whisper"]),
        ("py", vec!["-m", "pip", "install", "-U", "openai-whisper"]),
        ("pip", vec!["install", "-U", "openai-whisper"]),
    ];
    for (cmd, args) in pip_targets {
        if run_command(cmd, &args).is_ok() {
            notes.push(format!("{cmd} {}", args.join(" ")));
            return Ok(notes);
        }
        notes.push(format!("{cmd} {} (failed)", args.join(" ")));
    }
    Err(format!("Whisper install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(target_os = "macos")]
fn install_opencode_macos() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_brew_install("opencode", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_npm_global_install("opencode-ai", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("OpenCode install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install_opencode_linux() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_apt_install("opencode", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_dnf_install("opencode", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_pacman_install("opencode", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_npm_global_install("opencode-ai", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("OpenCode install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(target_os = "macos")]
fn install_tesseract_macos() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_brew_install("tesseract", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("Tesseract install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install_tesseract_linux() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_apt_install("tesseract-ocr", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_dnf_install("tesseract", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_pacman_install("tesseract", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("Tesseract install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(target_os = "macos")]
fn install_pandoc_macos() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_brew_install("pandoc", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("Pandoc install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install_pandoc_linux() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if try_apt_install("pandoc", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_dnf_install("pandoc", &mut notes).is_ok() {
        return Ok(notes);
    }
    if try_pacman_install("pandoc", &mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("Pandoc install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(unix)]
fn install_whisper_unix() -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    let _ = try_brew_install("ffmpeg", &mut notes);
    let _ = try_apt_install("ffmpeg", &mut notes);
    let _ = try_dnf_install("ffmpeg", &mut notes);
    let _ = try_pacman_install("ffmpeg", &mut notes);
    if try_npm_global_install("opencode-ai", &mut notes).is_ok() {
        // no-op; keeps npm availability in notes for support context
    }
    if run_pip_install_whisper(&mut notes).is_ok() {
        return Ok(notes);
    }
    Err(format!("Whisper install failed. Attempts: {}", notes.join(" | ")))
}

#[cfg(windows)]
fn try_winget_install(package_id: &str, notes: &mut Vec<String>) -> Result<(), String> {
    if !command_exists("winget") {
        notes.push("winget not available in PATH".to_string());
        return Err("winget not available in PATH".to_string());
    }
    let args = [
        "install",
        "--id",
        package_id,
        "--exact",
        "--silent",
        "--accept-package-agreements",
        "--accept-source-agreements",
    ];
    run_command("winget", &args)
        .map(|_| notes.push(format!("winget {}", args.join(" "))))
        .map_err(|err| {
            notes.push(format!("winget {} (failed: {err})", args.join(" ")));
            err
        })
}

#[cfg(windows)]
fn try_choco_install(package_name: &str, notes: &mut Vec<String>) -> Result<(), String> {
    if !command_exists("choco") {
        notes.push("choco not available in PATH".to_string());
        return Err("choco not available in PATH".to_string());
    }
    let args = [
        "install",
        package_name,
        "-y",
        "--limit-output",
        "--no-progress",
        "--accept-license",
    ];
    run_command("choco", &args)
        .map(|_| notes.push(format!("choco {}", args.join(" "))))
        .map_err(|err| {
            notes.push(format!("choco {} (failed: {err})", args.join(" ")));
            err
        })
}

fn run_command(command: &str, args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    sanitize_poison_proxy_env(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("{command} not available: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        Err(if detail.is_empty() {
            format!("{command} exited with status {}", output.status)
        } else {
            detail
        })
    }
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .is_ok()
}

#[cfg(windows)]
fn is_windows_elevated() -> bool {
    let script = "$p = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent()); if ($p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { exit 0 } else { exit 1 }";
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn windows_manual_tool_hint(tool_key: &str) -> &'static str {
    match tool_key {
        "tesseract" => "Install manually from an elevated terminal: `winget install --id UB-Mannheim.TesseractOCR --exact`.",
        "pandoc" => "Install manually from an elevated terminal: `winget install --id JohnMacFarlane.Pandoc --exact`.",
        _ => "Install manually from an elevated terminal.",
    }
}

fn sanitize_poison_proxy_env(cmd: &mut Command) {
    let proxy_keys = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "no_proxy",
        "GIT_HTTP_PROXY",
        "GIT_HTTPS_PROXY",
    ];
    for key in proxy_keys {
        if let Ok(value) = std::env::var(key) {
            let lower = value.to_ascii_lowercase();
            if lower.contains("127.0.0.1:9") || lower.contains("localhost:9") {
                cmd.env_remove(key);
            }
        }
    }
}

fn run_command_with_prefix(command: &str, args: &[&str], notes: &mut Vec<String>) -> Result<(), String> {
    run_command(command, args)
        .map(|_| notes.push(format!("{command} {}", args.join(" "))))
        .map_err(|err| {
            notes.push(format!("{command} {} (failed: {err})", args.join(" ")));
            err
        })
}

fn try_npm_global_install(package_name: &str, notes: &mut Vec<String>) -> Result<(), String> {
    run_command_with_prefix("npm", &["install", "-g", package_name], notes)
}

#[cfg(target_os = "macos")]
fn try_brew_install(package_name: &str, notes: &mut Vec<String>) -> Result<(), String> {
    run_command_with_prefix("brew", &["install", package_name], notes)
}

#[cfg(unix)]
fn try_apt_install(package_name: &str, notes: &mut Vec<String>) -> Result<(), String> {
    run_command_with_prefix(
        "sudo",
        &["apt-get", "install", "-y", package_name],
        notes,
    )
}

#[cfg(unix)]
fn try_dnf_install(package_name: &str, notes: &mut Vec<String>) -> Result<(), String> {
    run_command_with_prefix("sudo", &["dnf", "install", "-y", package_name], notes)
}

#[cfg(unix)]
fn try_pacman_install(package_name: &str, notes: &mut Vec<String>) -> Result<(), String> {
    run_command_with_prefix(
        "sudo",
        &["pacman", "-S", "--noconfirm", package_name],
        notes,
    )
}

#[cfg(unix)]
fn run_pip_install_whisper(notes: &mut Vec<String>) -> Result<(), String> {
    let candidates = [
        ("python", vec!["-m", "pip", "install", "-U", "openai-whisper"]),
        ("python3", vec!["-m", "pip", "install", "-U", "openai-whisper"]),
        ("py", vec!["-m", "pip", "install", "-U", "openai-whisper"]),
        ("pip", vec!["install", "-U", "openai-whisper"]),
        ("pip3", vec!["install", "-U", "openai-whisper"]),
    ];
    for (command, args) in candidates {
        if run_command_with_prefix(command, &args, notes).is_ok() {
            return Ok(());
        }
    }
    Err("no working pip command found".to_string())
}

fn tool_override_env_var(key: &str) -> Option<&'static str> {
    match key {
        "tesseract" => Some("EDUTRACK_TESSERACT_PATH"),
        "pandoc" => Some("EDUTRACK_PANDOC_PATH"),
        _ => None,
    }
}

fn tool_binary_filename(key: &str) -> String {
    #[cfg(windows)]
    {
        return format!("{key}.exe");
    }
    #[cfg(not(windows))]
    {
        key.to_string()
    }
}

fn bundled_tool_candidates(key: &str) -> Vec<PathBuf> {
    if !matches!(key, "tesseract" | "pandoc") {
        return Vec::new();
    }
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let binary_name = tool_binary_filename(key);
    let mut candidates = Vec::new();
    let mut roots = Vec::new();

    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    if let Ok(cwd) = env::current_dir() {
        roots.push(cwd.clone());
        roots.push(cwd.join("src-tauri"));
    }

    for root in roots {
        candidates.push(
            root.join("resources")
                .join("tools")
                .join(platform)
                .join(key)
                .join(&binary_name),
        );
        candidates.push(
            root.join("tools")
                .join(platform)
                .join(key)
                .join(&binary_name),
        );
    }
    candidates
}

fn resolve_bundled_tool(key: &str) -> Option<PathBuf> {
    if let Some(var_name) = tool_override_env_var(key) {
        if let Ok(raw_path) = env::var(var_name) {
            let override_path = PathBuf::from(raw_path);
            if override_path.exists() {
                return Some(override_path);
            }
        }
    }
    bundled_tool_candidates(key)
        .into_iter()
        .find(|path| path.exists())
}

fn command_for_tool(key: &str) -> (Command, String) {
    if let Some(path) = resolve_bundled_tool(key) {
        let display = path.to_string_lossy().to_string();
        return (Command::new(path), format!("bundled binary at {display}"));
    }
    (Command::new(key), "PATH".to_string())
}

fn probe_tool(key: &str, label: &str, args: &[&str]) -> GradingToolStatus {
    if key == "opencode" {
        return match probe_opencode_cli() {
            Ok(detail) => GradingToolStatus {
                key: key.to_string(),
                label: label.to_string(),
                available: true,
                detail,
            },
            Err(err) => GradingToolStatus {
                key: key.to_string(),
                label: label.to_string(),
                available: false,
                detail: format!("OpenCode probe failed: {err}"),
            },
        };
    }
    if key == "whisper" {
        return match probe_whisper_cli() {
            Ok(detail) => GradingToolStatus {
                key: key.to_string(),
                label: label.to_string(),
                available: true,
                detail,
            },
            Err(err) => GradingToolStatus {
                key: key.to_string(),
                label: label.to_string(),
                available: false,
                detail: err,
            },
        };
    }
    let (mut tool_command, source_label) = command_for_tool(key);
    match tool_command.args(args).output() {
        Ok(output) => {
            let detail = first_non_empty_line(&String::from_utf8_lossy(&output.stdout))
                .or_else(|| first_non_empty_line(&String::from_utf8_lossy(&output.stderr)))
                .unwrap_or_else(|| format!("{key} command responded ({source_label})"));
            GradingToolStatus {
                key: key.to_string(),
                label: label.to_string(),
                available: true,
                detail: if detail.contains("bundled binary at") {
                    detail
                } else {
                    format!("{detail} ({source_label})")
                },
            }
        }
        Err(err) => GradingToolStatus {
            key: key.to_string(),
            label: label.to_string(),
            available: false,
            detail: format!("{key} unavailable via {source_label} ({err})"),
        },
    }
}

fn run_whisper_transcription(path: &str, out_dir: &Path) -> Result<std::process::Output, String> {
    let mut attempts = Vec::new();
    let candidates: [(&str, &[&str]); 2] = [
        (
            "python",
            &[
                "-m",
                "whisper",
                path,
                "--model",
                "base",
                "--output_format",
                "txt",
                "--output_dir",
            ],
        ),
        (
            "py",
            &[
                "-m",
                "whisper",
                path,
                "--model",
                "base",
                "--output_format",
                "txt",
                "--output_dir",
            ],
        ),
    ];

    for (command, prefix_args) in candidates {
        let mut cmd = Command::new(command);
        cmd.args(prefix_args).arg(out_dir);
        match cmd.output() {
            Ok(output) => return Ok(output),
            Err(err) => attempts.push(format!("{command}: {err}")),
        }
    }

    Err(format!(
        "audio transcription requires Whisper via one of: `python -m whisper`, `py -m whisper`. Attempts: {}",
        attempts.join(" | ")
    ))
}

fn probe_whisper_cli() -> Result<String, String> {
    let candidates: [(&str, &[&str], &str); 2] = [
        ("python", &["-m", "whisper", "--help"], "python -m whisper"),
        ("py", &["-m", "whisper", "--help"], "py -m whisper"),
    ];
    let mut errors = Vec::new();
    for (command, args, label) in candidates {
        match Command::new(command).args(args).output() {
            Ok(output) if output.status.success() => {
                let detail = first_non_empty_line(&String::from_utf8_lossy(&output.stdout))
                    .or_else(|| first_non_empty_line(&String::from_utf8_lossy(&output.stderr)))
                    .unwrap_or_else(|| format!("{label} responded"));
                return Ok(format!("{label}: {detail}"));
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let detail = if !stderr.is_empty() { stderr } else { stdout };
                if looks_like_windows_cp1252_help_encoding_issue(&detail) {
                    return Ok(format!(
                        "{label}: detected (Windows console encoding issue while printing help text)"
                    ));
                }
                errors.push(format!("{label} exited {}", output.status));
                if !detail.is_empty() {
                    errors.push(format!("{label} output: {}", truncate_with_ellipsis(&detail, 220)));
                }
            }
            Err(err) => errors.push(format!("{label} not available ({err})")),
        }
    }
    Err(format!(
        "Whisper not available. Checked: python -m whisper, py -m whisper. {}",
        truncate_with_ellipsis(&errors.join(" | "), 420)
    ))
}

fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn truncate_with_ellipsis(value: &str, max_chars: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut truncated = String::new();
    for (index, ch) in normalized.chars().enumerate() {
        if index >= max_chars.saturating_sub(1) {
            break;
        }
        truncated.push(ch);
    }
    format!("{truncated}...")
}

fn looks_like_windows_cp1252_help_encoding_issue(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.contains("unicodeencodeerror")
        && lower.contains("cp1252")
        && lower.contains("argparse")
        && lower.contains("whisper")
}


#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE assessments (
                id TEXT PRIMARY KEY,
                class_id TEXT NOT NULL,
                title TEXT NOT NULL,
                assessment_date TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE students (
                id TEXT PRIMARY KEY,
                class_id TEXT NOT NULL,
                full_name TEXT NOT NULL,
                archived_at TEXT
            );
            CREATE TABLE assessment_scores (
                id TEXT PRIMARY KEY,
                assessment_id TEXT NOT NULL,
                student_id TEXT NOT NULL,
                score_value REAL,
                score_label TEXT,
                teacher_comment TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .expect("schema");
        conn
    }

    #[test]
    fn new_id_format() {
        let id = new_id("test");
        assert!(id.starts_with("test-"));
    }

    #[test]
    fn create_test_rows_seeds_only_active_students() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO students (id, class_id, full_name, archived_at) VALUES ('st-1', 'class-1', 'A', NULL)",
            [],
        )
        .expect("insert active");
        conn.execute(
            "INSERT INTO students (id, class_id, full_name, archived_at) VALUES ('st-2', 'class-1', 'B', '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("insert archived");

        let assessment_id = create_test_rows(
            &conn,
            "class-1",
            "Quiz 1",
            "Solve all items",
            "Use rubric",
            100.0,
        )
        .expect("create test");

        let description: String = conn
            .query_row(
                "SELECT description FROM assessments WHERE id = ?1",
                params![assessment_id],
                |row| row.get(0),
            )
            .expect("description");
        assert!(description.contains("INSTRUCTIONS:Solve all items"));
        assert!(description.contains("RUBRIC:Use rubric"));
        assert!(description.contains("MAX_SCORE:100"));

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM assessment_scores", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 1, "only active students should get score rows");

        let seeded_student: String = conn
            .query_row("SELECT student_id FROM assessment_scores", [], |row| row.get(0))
            .expect("seeded student");
        assert_eq!(seeded_student, "st-1");
    }

    #[test]
    fn update_submission_score_writes_score_label_and_comment() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO assessment_scores (id, assessment_id, student_id, score_value, score_label, teacher_comment, created_at, updated_at)
             VALUES ('score-1', 'test-1', 'st-1', NULL, NULL, NULL, 't', 't')",
            [],
        )
        .expect("insert score row");

        update_submission_score(
            &conn,
            "test-1",
            "st-1",
            87.5,
            Some("Good"),
            Some("Strong reasoning"),
        )
        .expect("update score");

        let (score, label, comment): (f64, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT score_value, score_label, teacher_comment FROM assessment_scores WHERE id = 'score-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query updated score");

        assert!((score - 87.5).abs() < f64::EPSILON);
        assert_eq!(label.as_deref(), Some("Good"));
        assert_eq!(comment.as_deref(), Some("Strong reasoning"));
    }
}
