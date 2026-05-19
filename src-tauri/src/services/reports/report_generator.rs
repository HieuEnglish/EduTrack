use crate::services::hierarchy::{get_class, new_id, now, DbState, Student};
use crate::services::llm_service::{generate_local_report, load_llm_config};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentReport {
    pub id: String,
    pub student_id: String,
    pub class_id: String,
    pub generated_from_plan_id: Option<String>,
    pub word_count_target: i32,
    pub report_text: String,
    pub teacher_advice_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentReportStatus {
    pub student_id: String,
    pub student_name: String,
    pub report_id: Option<String>,
    pub report_exists: bool,
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentDataBundle {
    pub student: Student,
    pub attendance_summary: String,
    pub assessment_summary: String,
    pub session_count: i32,
    pub attendance_rate: f64,
    pub avg_score: Option<f64>,
}

pub fn load_student(conn: &rusqlite::Connection, student_id: &str) -> Result<Student, String> {
    conn.query_row(
        "SELECT id, class_id, school_id, level_id, full_name, preferred_name, student_code, age, gender, avatar_seed, notes_private, created_at, updated_at, archived_at FROM students WHERE id = ?1",
        params![student_id],
        |row| Ok(Student {
            id: row.get(0)?, class_id: row.get(1)?, school_id: row.get(2)?,
            level_id: row.get(3)?, full_name: row.get(4)?, preferred_name: row.get(5)?,
            student_code: row.get(6)?, age: row.get(7)?, gender: row.get(8)?,
            avatar_seed: row.get(9)?, notes_private: row.get(10)?,
            created_at: row.get(11)?, updated_at: row.get(12)?, archived_at: row.get(13)?,
        }),
    ).map_err(|e| e.to_string())
}

pub fn load_report(conn: &rusqlite::Connection, report_id: &str) -> Result<StudentReport, String> {
    conn.query_row(
        "SELECT id, student_id, class_id, generated_from_plan_id, word_count_target, report_text, teacher_advice_text, created_at, updated_at FROM student_reports WHERE id = ?1",
        params![report_id],
        |row| Ok(StudentReport {
            id: row.get(0)?, student_id: row.get(1)?, class_id: row.get(2)?,
            generated_from_plan_id: row.get(3)?, word_count_target: row.get(4)?,
            report_text: row.get(5)?, teacher_advice_text: row.get(6)?,
            created_at: row.get(7)?, updated_at: row.get(8)?,
        }),
    ).map_err(|e| e.to_string())
}

// Legacy commands kept for backward compatibility

#[tauri::command]
pub async fn generate_student_report(
    state: DbState<'_>,
    student_id: String,
    class_id: String,
    word_count_target: Option<i32>,
    _generated_from_plan_id: Option<String>,
) -> Result<String, String> {
    generate_student_report_v2(state, student_id, class_id, None, word_count_target).await
}

#[tauri::command]
pub async fn get_student_report(
    state: DbState<'_>,
    report_id: String,
) -> Result<StudentReport, String> {
    state.with_conn(|conn| load_report(conn, &report_id))
}

#[tauri::command]
pub async fn update_student_report(
    state: DbState<'_>,
    report_id: String,
    report_text: String,
    teacher_advice_text: Option<String>,
) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE student_reports SET report_text = ?1, teacher_advice_text = ?2, updated_at = ?3 WHERE id = ?4",
            params![report_text, teacher_advice_text, now(), report_id],
        ).map_err(|e| e.to_string())?;
        Ok(())
    })
}

// New commands

#[tauri::command]
pub async fn get_class_report_status(
    state: DbState<'_>,
    class_id: String,
) -> Result<Vec<StudentReportStatus>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT s.id, s.full_name, sr.id, sr.updated_at
             FROM students s LEFT JOIN student_reports sr ON sr.student_id = s.id
             WHERE s.class_id = ?1 AND s.archived_at IS NULL ORDER BY s.full_name"
        ).map_err(|e| e.to_string())?;
        let result = stmt.query_map(params![class_id], |row| {
            let report_id: Option<String> = row.get(2)?;
            Ok(StudentReportStatus {
                student_id: row.get(0)?, student_name: row.get(1)?,
                report_id: report_id.clone(), report_exists: report_id.is_some(),
                last_updated: row.get(3)?,
            })
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn get_student_data_bundle(
    state: DbState<'_>,
    student_id: String,
) -> Result<StudentDataBundle, String> {
    state.with_conn(|conn| get_student_data_bundle_internal(conn, &student_id))
}

#[tauri::command]
pub async fn generate_student_report_v2(
    state: DbState<'_>,
    student_id: String,
    class_id: String,
    teacher_instructions: Option<String>,
    word_count_target: Option<i32>,
) -> Result<String, String> {
    state.with_conn(|conn| {
        let student = load_student(conn, &student_id)?;
        let class = get_class(conn, &class_id)?;
        let bundle = get_student_data_bundle_internal(conn, &student_id)?;
        let target = word_count_target.unwrap_or(300).clamp(80, 2000);
        let teacher_note = teacher_instructions.as_deref().unwrap_or("Focus on strengths, growth areas, and next steps.");

        let student_info = format!(
            "Student: {name}\nAge: {age}\nGender: {gender}",
            name = student.full_name,
            age = student.age.map(|a| a.to_string()).unwrap_or_else(|| "Not specified".to_string()),
            gender = student.gender.as_deref().unwrap_or("Not specified"),
        );

        let notes = student.notes_private.as_deref().filter(|n| !n.trim().is_empty()).map(|n| format!("\nTeacher notes about this student: {}", n)).unwrap_or_default();

        let prompt = format!(
            r#"Write a professional student report of about {target} words.

{student_info}
Class: {class}
Subject: {subject}

{bundle}{notes}

Teacher instructions: {teacher_note}

Write in paragraphs: academic progress, strengths, areas for growth, and recommendations. Do not invent specific scores."#,
            student_info = student_info, class = class.name,
            subject = class.subject_name.as_deref().unwrap_or("General Studies"),
            bundle = format!("{}\n{}", bundle.attendance_summary, bundle.assessment_summary),
            teacher_note = teacher_note,
        );

        let config = load_llm_config(conn);
        let report_text = match generate_local_report(&config, &prompt) {
            Ok(text) => {
                let trimmed = text.trim().to_string();
                if trimmed.len() < 20 || trimmed.len() > 100_000 {
                    let avg = bundle.avg_score.map(|s| format!("{s:.1}")).unwrap_or_else(|| "N/A".to_string());
                    let rate = format!("{:.0}%", bundle.attendance_rate);
                    format!(
                        "{name} has shown progress in {subject} this term. Attendance is at {rate} with {sessions} recorded sessions. Average assessment score is {avg}. {name} engages with class activities and is developing confidence with the curriculum. Next term, continued focus and targeted practice will support further growth.",
                        name = student.full_name, subject = class.subject_name.as_deref().unwrap_or("the subject"),
                        rate = rate, sessions = bundle.session_count, avg = avg,
                    )
                } else {
                    trimmed
                }
            }
            Err(_) => {
                let avg = bundle.avg_score.map(|s| format!("{s:.1}")).unwrap_or_else(|| "N/A".to_string());
                let rate = format!("{:.0}%", bundle.attendance_rate);
                format!(
                    "{name} has shown progress in {subject} this term. Attendance is at {rate} with {sessions} recorded sessions. Average assessment score is {avg}. {name} engages with class activities and is developing confidence with the curriculum. Next term, continued focus and targeted practice will support further growth.",
                    name = student.full_name, subject = class.subject_name.as_deref().unwrap_or("the subject"),
                    rate = rate, sessions = bundle.session_count, avg = avg,
                )
            }
        };

        let advice = Some(format!(
            "Teacher: Review this draft, add specific examples from classroom observations, and adjust the recommendations. Prioritize 2-3 next steps for {}.",
            student.preferred_name.as_deref().unwrap_or(&student.full_name)
        ));

        let report_id = new_id("report");
        let timestamp = now();

        conn.execute("DELETE FROM student_reports WHERE student_id = ?1 AND class_id = ?2",
            params![student_id, class_id]).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO student_reports (id, student_id, class_id, generated_from_plan_id, word_count_target, report_text, teacher_advice_text, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![report_id, student_id, class_id, None::<String>, target, report_text, advice, timestamp, timestamp],
        ).map_err(|e| e.to_string())?;

        Ok(report_id)
    })
}

fn get_student_data_bundle_internal(conn: &rusqlite::Connection, student_id: &str) -> Result<StudentDataBundle, String> {
    let student = load_student(conn, student_id)?;
    let total_sessions: i32 = conn.query_row(
        "SELECT COUNT(*) FROM sessions s JOIN students st ON st.class_id = s.class_id WHERE st.id = ?1",
        params![student_id], |row| row.get(0),
    ).unwrap_or(0);
    let absences: i32 = conn.query_row(
        "SELECT COUNT(*) FROM attendance_records ar WHERE ar.student_id = ?1 AND ar.status = 'absent'",
        params![student_id], |row| row.get(0),
    ).unwrap_or(0);
    let lates: i32 = conn.query_row(
        "SELECT COUNT(*) FROM attendance_records ar WHERE ar.student_id = ?1 AND ar.status = 'late'",
        params![student_id], |row| row.get(0),
    ).unwrap_or(0);
    let presents: i32 = conn.query_row(
        "SELECT COUNT(*) FROM attendance_records ar WHERE ar.student_id = ?1 AND ar.status = 'present'",
        params![student_id], |row| row.get(0),
    ).unwrap_or(0);
    let total_attended = presents + lates;
    let attendance_rate = if total_sessions > 0 { (total_attended as f64 / total_sessions as f64) * 100.0 } else { 0.0 };
    let attendance_summary = format!("Attendance: {presents} present, {lates} late, {absences} absent across {total_sessions} sessions ({attendance_rate:.0}% rate).");

    let avg_score: Option<f64> = conn.query_row(
        "SELECT AVG(as2.score_value) FROM assessment_scores as2 WHERE as2.student_id = ?1 AND as2.score_value IS NOT NULL",
        params![student_id], |row| row.get(0),
    ).ok();

    let recent: Vec<(String, f64)> = {
        let mut stmt = conn.prepare(
            "SELECT a.title, COALESCE(as2.score_value, 0) FROM assessment_scores as2
             JOIN assessments a ON as2.assessment_id = a.id
             WHERE as2.student_id = ?1 AND as2.score_value IS NOT NULL
             ORDER BY a.assessment_date DESC LIMIT 5"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![student_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        }).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };

    let assessment_summary = if recent.is_empty() {
        "No assessment scores recorded yet.".to_string()
    } else {
        let mut s = format!("Recent assessments (average: {:.1}):", avg_score.unwrap_or(0.0));
        for (title, score) in &recent {
            s.push_str(&format!("\n- {title}: {score}"));
        }
        s
    };

    Ok(StudentDataBundle { student, attendance_summary, assessment_summary, session_count: total_sessions, attendance_rate, avg_score })
}
