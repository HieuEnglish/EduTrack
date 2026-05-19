use crate::services::hierarchy::{new_id, now, DbState};
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentAssignment {
    pub agent_type: String,
    pub enabled: bool,
    pub config_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentInsight {
    pub id: String,
    pub class_id: String,
    pub student_id: Option<String>,
    pub agent_type: String,
    pub severity: String,
    pub title: String,
    pub body: String,
    pub source_event_type: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub async fn update_class_agents(
    state: DbState<'_>,
    class_id: String,
    agents: Vec<AgentAssignment>,
) -> Result<(), String> {
    state.with_conn(|conn| {
        conn.execute(
            "DELETE FROM class_agent_assignments WHERE class_id = ?1",
            params![class_id],
        )
        .map_err(|e| e.to_string())?;
        for agent in agents {
            conn.execute(
                "INSERT INTO class_agent_assignments (id, class_id, agent_type, enabled, config_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    new_id("agent"),
                    class_id,
                    agent.agent_type,
                    agent.enabled,
                    agent.config_json,
                    now(),
                    now()
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

#[tauri::command]
pub async fn run_class_agents(
    state: DbState<'_>,
    class_id: String,
) -> Result<Vec<AgentInsight>, String> {
    state.with_conn(|conn| {
        let enabled = enabled_agents(conn, &class_id)?;
        let mut insights = Vec::new();
        if enabled.iter().any(|agent| agent == "attendance") {
            insights.extend(attendance_insights(conn, &class_id)?);
        }
        if enabled.iter().any(|agent| agent == "planning") {
            insights.extend(planning_insights(conn, &class_id)?);
        }

        if enabled.iter().any(|agent| agent == "performance") {
            insights.extend(performance_insights(conn, &class_id)?);
        }
        if enabled.iter().any(|agent| agent == "assignments") {
            insights.extend(assignments_insights(conn, &class_id)?);
        }
        if enabled.iter().any(|agent| agent == "engagement") {
            insights.extend(engagement_insights(conn, &class_id)?);
        }
        if enabled.iter().any(|agent| agent == "reports") {
            insights.extend(report_writer_insights(conn, &class_id)?);
        }
        if enabled.iter().any(|agent| agent == "syllabus") {
            insights.extend(syllabus_extraction_insights(conn, &class_id)?);
        }
        conn.execute(
            "DELETE FROM agent_insights WHERE class_id = ?1",
            params![class_id],
        )
        .map_err(|e| e.to_string())?;
        for insight in &insights {
            conn.execute(
                "INSERT INTO agent_insights (id, class_id, student_id, agent_type, severity, title, body, source_event_type, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    insight.id,
                    insight.class_id,
                    insight.student_id,
                    insight.agent_type,
                    insight.severity,
                    insight.title,
                    insight.body,
                    insight.source_event_type,
                    insight.created_at
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(insights)
    })
}

fn enabled_agents(conn: &rusqlite::Connection, class_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT agent_type FROM class_agent_assignments WHERE class_id = ?1 AND enabled = 1",
        )
        .map_err(|e| e.to_string())?;
    let agents = stmt
        .query_map(params![class_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;
    if agents.is_empty() {
        Ok(vec![
            "attendance".to_string(),
            "planning".to_string(),
            "performance".to_string(),
            "assignments".to_string(),
            "engagement".to_string(),
            "reports".to_string(),
            "syllabus".to_string(),
        ])
    } else {
        Ok(agents)
    }
}

fn attendance_insights(
    conn: &rusqlite::Connection,
    class_id: &str,
) -> Result<Vec<AgentInsight>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.full_name,
                    SUM(CASE WHEN ar.status = 'absent' THEN 1 ELSE 0 END) AS absences,
                    SUM(CASE WHEN ar.status != 'ignore' THEN 1 ELSE 0 END) AS total
             FROM students s
             LEFT JOIN attendance_records ar ON ar.student_id = s.id
             WHERE s.class_id = ?1 AND s.archived_at IS NULL
             GROUP BY s.id, s.full_name
             HAVING total >= 3 AND absences * 1.0 / total >= 0.25",
        )
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map(params![class_id], |row| {
            let student_id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let absences: i32 = row.get(2)?;
            let total: i32 = row.get(3)?;
            Ok(AgentInsight {
                id: new_id("insight"),
                class_id: class_id.to_string(),
                student_id: Some(student_id),
                agent_type: "attendance".to_string(),
                severity: "warning".to_string(),
                title: format!("{name} attendance needs review"),
                body: format!("{name} has {absences} absences across {total} recorded sessions."),
                source_event_type: Some("attendance_records".to_string()),
                created_at: now(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

fn planning_insights(
    conn: &rusqlite::Connection,
    class_id: &str,
) -> Result<Vec<AgentInsight>, String> {
    let overdue: i32 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM year_plan_lessons ypl
             JOIN classes c ON c.year_plan_id = ypl.year_plan_id
             WHERE c.id = ?1 AND ypl.status = 'planned' AND ypl.teaching_date < date('now')",
            params![class_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if overdue == 0 {
        return Ok(Vec::new());
    }
    Ok(vec![AgentInsight {
        id: new_id("insight"),
        class_id: class_id.to_string(),
        student_id: None,
        agent_type: "planning".to_string(),
        severity: if overdue > 5 { "high" } else { "warning" }.to_string(),
        title: "Plan drift detected".to_string(),
        body: format!("{overdue} planned lessons are dated before today and still not completed."),
        source_event_type: Some("year_plan_lessons".to_string()),
        created_at: now(),
    }])
}

const PERFORMANCE_DECLINE_POINTS: f64 = 5.0;

fn performance_insights(
    conn: &rusqlite::Connection,
    class_id: &str,
) -> Result<Vec<AgentInsight>, String> {
    let mut insights = Vec::new();
    let recent: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM assessments a
             JOIN classes c ON a.class_id = c.id
             WHERE c.id = ?1 AND a.assessment_date > date('now', '-30 days')",
            params![class_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if recent == 0 {
        let total: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM assessments WHERE class_id = ?1",
                params![class_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if total > 0 && total < 3 {
            insights.push(AgentInsight {
                id: new_id("insight"),
                class_id: class_id.to_string(),
                student_id: None,
                agent_type: "performance".to_string(),
                severity: "warning".to_string(),
                title: "Limited recent assessment data".to_string(),
                body: format!("Only {total} assessments recorded. Add more assessments to track academic performance trends."),
                source_event_type: Some("assessments".to_string()),
                created_at: now(),
            });
        }
        return Ok(insights);
    }
    let sql = format!(
        "SELECT COUNT(*) FROM (
            SELECT as2.student_id,
                   AVG(CASE WHEN a.assessment_date >= date('now', '-14 days') THEN as2.score_value END) as recent_avg,
                   AVG(CASE WHEN a.assessment_date < date('now', '-14 days') THEN as2.score_value END) as past_avg
            FROM assessment_scores as2
            JOIN assessments a ON as2.assessment_id = a.id
            WHERE a.class_id = ?1 AND as2.score_value IS NOT NULL
            GROUP BY as2.student_id
            HAVING past_avg IS NOT NULL AND recent_avg IS NOT NULL AND recent_avg < past_avg - {PERFORMANCE_DECLINE_POINTS}
        )"
    );
    let declining: i32 = conn
        .query_row(&sql, params![class_id], |row| row.get(0))
        .unwrap_or(0);
    if declining > 0 {
        let s = if declining == 1 { "" } else { "s" };
        let has = if declining == 1 { " shows" } else { " show" };
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "performance".to_string(),
            severity: if declining > 3 { "high" } else { "warning" }.to_string(),
            title: format!("{declining} student{s}{has} declining assessment scores"),
            body: format!("{declining} student{s} had lower scores in recent assessments compared to earlier work. Consider reviewing concepts with these students."),
            source_event_type: Some("assessment_scores".to_string()),
            created_at: now(),
        });
    }
    Ok(insights)
}

fn assignments_insights(
    conn: &rusqlite::Connection,
    class_id: &str,
) -> Result<Vec<AgentInsight>, String> {
    let mut insights = Vec::new();
    let missing: i32 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM sessions s
             JOIN students st ON st.class_id = s.class_id AND st.archived_at IS NULL
             LEFT JOIN attendance_records ar ON ar.session_id = s.id AND ar.student_id = st.id
             WHERE s.class_id = ?1 AND s.completed = 1 AND ar.id IS NULL
             GROUP BY s.id
             HAVING COUNT(*) >= 1",
            params![class_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if missing > 0 {
        let s = if missing == 1 { "" } else { "s" };
        let verb = if missing == 1 { " has" } else { "s have" };
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "assignments".to_string(),
            severity: if missing > 5 { "high" } else { "warning" }.to_string(),
            title: format!("{missing} completed session{s} with missing attendance records"),
            body: format!("{missing} completed session{verb} no attendance records for some students. Review and fill in the gaps."),
            source_event_type: Some("attendance_records".to_string()),
            created_at: now(),
        });
    }
    Ok(insights)
}

fn engagement_insights(
    conn: &rusqlite::Connection,
    class_id: &str,
) -> Result<Vec<AgentInsight>, String> {
    let mut insights = Vec::new();
    let total_sessions: i32 = conn.query_row("SELECT COUNT(*) FROM sessions WHERE class_id = ?1", params![class_id], |row| row.get(0)).unwrap_or(0);
    let completed_sessions: i32 = conn.query_row("SELECT COUNT(*) FROM sessions WHERE class_id = ?1 AND completed = 1", params![class_id], |row| row.get(0)).unwrap_or(0);
    if total_sessions > 0 && (completed_sessions as f64) / (total_sessions as f64) < 0.5 {
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "engagement".to_string(),
            severity: "warning".to_string(),
            title: "Low session completion rate".to_string(),
            body: format!("Only {completed_sessions} of {total_sessions} sessions ({:.0}%) are marked complete.", (completed_sessions as f64 / total_sessions as f64) * 100.0),
            source_event_type: Some("sessions".to_string()),
            created_at: now(),
        });
    }
    let late_pattern: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM (
                SELECT s.id FROM sessions s
                JOIN attendance_records ar ON ar.session_id = s.id
                WHERE s.class_id = ?1 AND ar.status = 'late'
                GROUP BY s.id HAVING COUNT(*) > 0
            )",
            params![class_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if late_pattern > 0 {
        let s = if late_pattern == 1 { "" } else { "s" };
        let verb = if late_pattern == 1 { " has" } else { "s have" };
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "engagement".to_string(),
            severity: "neutral".to_string(),
            title: format!("{late_pattern} session{s} had late arrivals"),
            body: format!("{late_pattern} session{verb} recorded late arrivals. Monitor for recurring patterns."),
            source_event_type: Some("attendance_records".to_string()),
            created_at: now(),
        });
    }
    Ok(insights)
}

fn report_writer_insights(
    conn: &rusqlite::Connection,
    class_id: &str,
) -> Result<Vec<AgentInsight>, String> {
    let mut insights = Vec::new();
    let student_count: i32 = conn
        .query_row("SELECT COUNT(*) FROM students WHERE class_id = ?1 AND archived_at IS NULL", params![class_id], |row| row.get(0))
        .unwrap_or(0);
    let report_count: i32 = conn
        .query_row("SELECT COUNT(*) FROM student_reports WHERE class_id = ?1", params![class_id], |row| row.get(0))
        .unwrap_or(0);
    if student_count > 0 && report_count == 0 {
        let s = if student_count == 1 { "" } else { "s" };
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "reports".to_string(),
            severity: "neutral".to_string(),
            title: "Ready to generate reports".to_string(),
            body: format!("{student_count} student{s}. No reports yet. Generate reports when you have enough assessment data."),
            source_event_type: Some("student_reports".to_string()),
            created_at: now(),
        });
    } else if student_count > 0 && report_count < student_count {
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "reports".to_string(),
            severity: "warning".to_string(),
            title: format!("{}/{} students have reports", report_count, student_count),
            body: format!("{} of {} students still need reports generated.", student_count - report_count, student_count),
            source_event_type: Some("student_reports".to_string()),
            created_at: now(),
        });
    } else if student_count > 0 {
        let outdated: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM student_reports sr
                 WHERE sr.class_id = ?1 AND sr.updated_at < date('now', '-30 days')",
                params![class_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if outdated > 0 {
            let s = if outdated == 1 { "" } else { "s" };
            let verb = if outdated == 1 { " needs" } else { " need" };
            let have = if outdated == 1 { " has" } else { "s have" };
            insights.push(AgentInsight {
                id: new_id("insight"),
                class_id: class_id.to_string(),
                student_id: None,
                agent_type: "reports".to_string(),
                severity: "warning".to_string(),
                title: format!("{outdated} report{s}{verb} updating"),
                body: format!("{outdated} report{have} been updated in over 30 days."),
                source_event_type: Some("student_reports".to_string()),
                created_at: now(),
            });
        }
    }
    Ok(insights)
}

fn syllabus_extraction_insights(
    conn: &rusqlite::Connection,
    class_id: &str,
) -> Result<Vec<AgentInsight>, String> {
    let mut insights = Vec::new();
    let syllabus: Option<(String, String, String)> = conn
        .query_row(
            "SELECT sd.id, sd.coverage_notes, COALESCE(sd.llm_extraction_status, '')
             FROM syllabus_documents sd
             JOIN levels l ON l.id = sd.level_id
             JOIN classes c ON c.level_id = l.id
             WHERE c.id = ?1 AND sd.is_active = 1
             ORDER BY sd.created_at DESC LIMIT 1",
            params![class_id],
            |row| {
                let id: String = row.get(0)?;
                let text: String = row.get(1).unwrap_or_default();
                let status: String = row.get(2)?;
                Ok((id, text, status))
            },
        )
        .ok();
    let Some((syllabus_id, raw_text, status)) = syllabus else {
        return Ok(vec![AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: "neutral".to_string(),
            title: "No syllabus uploaded".to_string(),
            body: "No syllabus document has been uploaded for this class. Upload a syllabus to enable extraction.".to_string(),
            source_event_type: Some("syllabus_documents".to_string()),
            created_at: now(),
        }]);
    };
    if raw_text.trim().is_empty() {
        return Ok(vec![AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: "warning".to_string(),
            title: "Syllabus text is empty".to_string(),
            body: "The uploaded syllabus has no extractable text. Try uploading a clearer PDF or text file.".to_string(),
            source_event_type: Some("syllabus_documents".to_string()),
            created_at: now(),
        }]);
    }
    let units: Vec<(String, String, i32)> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, title, COALESCE(estimated_lessons, 0) FROM curriculum_units WHERE syllabus_id = ?1 ORDER BY recommended_sequence",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_map(params![syllabus_id], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let lessons: i32 = row.get(2)?;
                Ok((id, title, lessons))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string());
        result
    }?;

    if units.is_empty() {
        if status == "failed" {
            insights.push(AgentInsight {
                id: new_id("insight"),
                class_id: class_id.to_string(),
                student_id: None,
                agent_type: "syllabus".to_string(),
                severity: "high".to_string(),
                title: "Syllabus extraction failed".to_string(),
                body: "The LLM was unable to extract units from this syllabus. Check provider billing/authentication and selected model in Settings, then retry.".to_string(),
                source_event_type: Some("syllabus_documents".to_string()),
                created_at: now(),
            });
        } else {
            insights.push(AgentInsight {
                id: new_id("insight"),
                class_id: class_id.to_string(),
                student_id: None,
                agent_type: "syllabus".to_string(),
                severity: "warning".to_string(),
                title: "No extracted units".to_string(),
                body: "The syllabus has been uploaded but no units were extracted. Run extraction again.".to_string(),
                source_event_type: Some("syllabus_documents".to_string()),
                created_at: now(),
            });
        }
        return Ok(insights);
    }

    let source_lower = raw_text.to_ascii_lowercase();
    let mut orphan_count = 0;
    let mut blocked_count = 0;
    let mut continuation_count = 0;
    let mut write_variant_pairs = 0;
    let blocked_re = Regex::new(
        r"(?i)^(at the end of the unit|students will be able to|learning objective|success criteria)",
    )
    .expect("valid blocked regex");
    let continuation_re =
        Regex::new(r"(?i)\((?:continuation|continued)\)").expect("valid continuation regex");
    let unit_prefix_re = Regex::new(r"(?i)^\s*unit\s*\d{1,2}\s*[:.\-]?\s*")
        .expect("valid unit prefix regex");

    let mut seen_cores: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for (_, title, _) in &units {
        let cleaned = title.trim();
        if cleaned.is_empty() || blocked_re.is_match(cleaned) {
            blocked_count += 1;
            continue;
        }
        if continuation_re.is_match(cleaned) {
            continuation_count += 1;
            continue;
        }
        let stripped = unit_prefix_re.replace(cleaned, "").trim().to_string();
        let lower = stripped.to_ascii_lowercase();
        let is_write = lower.starts_with("write ") || lower.starts_with("writing ");
        let core = if is_write {
            if lower.starts_with("write a ") {
                stripped[8..].trim().to_ascii_lowercase()
            } else if lower.starts_with("write an ") {
                stripped[9..].trim().to_ascii_lowercase()
            } else if lower.starts_with("write the ") {
                stripped[10..].trim().to_ascii_lowercase()
            } else if lower.starts_with("writing a ") {
                stripped[10..].trim().to_ascii_lowercase()
            } else if lower.starts_with("writing ") {
                stripped[8..].trim().to_ascii_lowercase()
            } else {
                stripped[6..].trim().to_ascii_lowercase()
            }
        } else {
            stripped.to_ascii_lowercase()
        };
        if is_write && seen_cores.contains_key(&core) {
            write_variant_pairs += 1;
        }
        seen_cores.insert(core, true);

        let title_tokens: Vec<&str> = cleaned
            .split_whitespace()
            .filter(|t| t.len() >= 4)
            .filter(|t| !matches!(*t, "Unit" | "unit" | "Write" | "write" | "Essay" | "Story" | "Lesson" | "lessons" | "Module" | "Topic"))
            .collect();
        let has_evidence = if title_tokens.is_empty() {
            false
        } else {
            let hits = title_tokens
                .iter()
                .filter(|t| source_lower.contains(&t.to_ascii_lowercase()))
                .count();
            hits >= 2 || (title_tokens.len() == 1 && hits == 1)
        };
        if !has_evidence {
            orphan_count += 1;
        }
    }

    let total_lessons: i32 = units.iter().map(|(_, _, l)| l).sum();
    let unit_count = units.len();

    if unit_count < 3 {
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: "warning".to_string(),
            title: format!("Only {unit_count} units extracted"),
            body: format!("Expected more instructional units. The syllabus lists {unit_count} units — review and add missing ones."),
            source_event_type: Some("curriculum_units".to_string()),
            created_at: now(),
        });
    } else if unit_count > 20 {
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: "warning".to_string(),
            title: format!("Unusual number of units ({unit_count})"),
            body: format!("{unit_count} units seems high. Verify that individual lessons weren't split into separate units."),
            source_event_type: Some("curriculum_units".to_string()),
            created_at: now(),
        });
    }

    if total_lessons == 0 {
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: "warning".to_string(),
            title: "Missing lesson estimates".to_string(),
            body: "Some units have no estimated lesson count. Set lesson estimates per unit to enable accurate scheduling.".to_string(),
            source_event_type: Some("curriculum_units".to_string()),
            created_at: now(),
        });
    }

    if orphan_count > 0 {
        let severity = if orphan_count as f64 / unit_count as f64 > 0.5 {
            "high"
        } else {
            "warning"
        };
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: severity.to_string(),
            title: format!("{orphan_count} unit title{} may not match syllabus", if orphan_count == 1 { "" } else { "s" }),
            body: format!("{orphan_count} of {unit_count} unit title{} had no clear match in the syllabus text. Review and rename if needed.", if orphan_count == 1 { "s" } else { "s" }),
            source_event_type: Some("curriculum_units".to_string()),
            created_at: now(),
        });
    }

    if continuation_count > 0 {
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: "neutral".to_string(),
            title: format!("{continuation_count} continuation entr{}", if continuation_count == 1 { "y" } else { "ies" }),
            body: format!("{continuation_count} unit{} marked as continuation. Verify pacing covers both semesters.", if continuation_count == 1 { " is" } else { "s are" }),
            source_event_type: Some("curriculum_units".to_string()),
            created_at: now(),
        });
    }

    if write_variant_pairs > 0 {
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: "warning".to_string(),
            title: "Duplicate write-variant titles".to_string(),
            body: format!("{write_variant_pairs} unit{0} appear{1} both with and without 'Write a...' prefix. Remove the redundant variant.", "", if write_variant_pairs == 1 { "s" } else { "" }),
            source_event_type: Some("curriculum_units".to_string()),
            created_at: now(),
        });
    }

    let total: i32 = conn
        .query_row("SELECT COUNT(*) FROM students WHERE class_id = ?1 AND archived_at IS NULL", params![class_id], |row| row.get(0))
        .unwrap_or(0);
    if unit_count >= 3 && orphan_count == 0 && blocked_count == 0 && total_lessons > 0 {
        insights.push(AgentInsight {
            id: new_id("insight"),
            class_id: class_id.to_string(),
            student_id: None,
            agent_type: "syllabus".to_string(),
            severity: "neutral".to_string(),
            title: "Syllabus extraction looks good".to_string(),
            body: format!("{unit_count} units extracted with {total_lessons} total estimated lessons across {total} students."),
            source_event_type: Some("curriculum_units".to_string()),
            created_at: now(),
        });
    }

    Ok(insights)
}

#[tauri::command]
pub async fn get_agent_insights(
    state: DbState<'_>,
    class_id: String,
) -> Result<Vec<AgentInsight>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, class_id, student_id, agent_type, severity, title, body, source_event_type, created_at FROM agent_insights WHERE class_id = ?1 ORDER BY created_at DESC LIMIT 50")
            .map_err(|e| e.to_string())?;
        let result = stmt.query_map(params![class_id], |row| {
            Ok(AgentInsight {
                id: row.get(0)?,
                class_id: row.get(1)?,
                student_id: row.get(2)?,
                agent_type: row.get(3)?,
                severity: row.get(4)?,
                title: row.get(5)?,
                body: row.get(6)?,
                source_event_type: row.get(7)?,
                created_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
        result
    })
}
