use crate::services::hierarchy::{get_class, new_id, now, DbState};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassScheduleRule {
    pub id: String,
    pub class_id: String,
    pub weekday: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub period_label: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub fn weekday_to_index(weekday: &str) -> Result<u32, String> {
    match weekday.to_ascii_lowercase().as_str() {
        "mon" | "monday" => Ok(0),
        "tue" | "tuesday" => Ok(1),
        "wed" | "wednesday" => Ok(2),
        "thu" | "thursday" => Ok(3),
        "fri" | "friday" => Ok(4),
        "sat" | "saturday" => Ok(5),
        "sun" | "sunday" => Ok(6),
        other => Err(format!("invalid weekday '{other}'")),
    }
}

pub fn class_weekday_set(
    conn: &rusqlite::Connection,
    class_id: Option<&str>,
) -> Result<Option<HashSet<u32>>, String> {
    let Some(class_id) = class_id else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare("SELECT weekday FROM class_schedule_rules WHERE class_id = ?1 AND is_active = 1")
        .map_err(|e| e.to_string())?;
    let weekdays: Vec<String> = stmt
        .query_map(params![class_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if weekdays.is_empty() {
        return Ok(None);
    }
    weekdays
        .iter()
        .map(|day| weekday_to_index(day))
        .collect::<Result<HashSet<_>, _>>()
        .map(Some)
}

fn summary_for_rules(rules: &[ClassScheduleRule]) -> String {
    let mut labels: Vec<String> = rules
        .iter()
        .map(|rule| {
            let mut label = rule.weekday.clone();
            if let Some(period) = &rule.period_label {
                if !period.trim().is_empty() {
                    label.push_str(&format!(" {period}"));
                }
            }
            label
        })
        .collect();
    labels.sort();
    labels.join(", ")
}

#[tauri::command]
pub async fn set_class_schedule_rules(
    state: DbState<'_>,
    class_id: String,
    rules: Vec<ClassScheduleRule>,
) -> Result<Vec<ClassScheduleRule>, String> {
    if rules.is_empty() {
        return Err("at least one meeting day is required".to_string());
    }
    for rule in &rules {
        weekday_to_index(&rule.weekday)?;
    }

    state.with_conn(|conn| {
        let class = get_class(conn, &class_id)?;
        conn.execute(
            "UPDATE class_schedule_rules SET is_active = 0, updated_at = ?1 WHERE class_id = ?2",
            params![now(), class_id],
        )
        .map_err(|e| e.to_string())?;

        let timestamp = now();
        let mut saved = Vec::with_capacity(rules.len());
        for mut rule in rules {
            rule.id = if rule.id.is_empty() {
                new_id("schedule")
            } else {
                rule.id
            };
            rule.class_id = class.id.clone();
            rule.is_active = true;
            rule.created_at = timestamp.clone();
            rule.updated_at = timestamp.clone();
            conn.execute(
                "INSERT INTO class_schedule_rules (id, class_id, weekday, start_time, end_time, period_label, is_active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
                params![
                    rule.id,
                    rule.class_id,
                    rule.weekday,
                    rule.start_time,
                    rule.end_time,
                    rule.period_label,
                    rule.created_at,
                    rule.updated_at
                ],
            )
            .map_err(|e| e.to_string())?;
            saved.push(rule);
        }
        let summary = summary_for_rules(&saved);
        conn.execute(
            "UPDATE classes SET schedule_pattern_summary = ?1, updated_at = ?2 WHERE id = ?3",
            params![summary, now(), class_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(saved)
    })
}

#[tauri::command]
pub async fn get_class_schedule_rules(
    state: DbState<'_>,
    class_id: String,
) -> Result<Vec<ClassScheduleRule>, String> {
    state.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, class_id, weekday, start_time, end_time, period_label, is_active, created_at, updated_at FROM class_schedule_rules WHERE class_id = ?1 AND is_active = 1 ORDER BY weekday, period_label")
            .map_err(|e| e.to_string())?;
        let result = stmt.query_map(params![class_id], |row| {
            Ok(ClassScheduleRule {
                id: row.get(0)?,
                class_id: row.get(1)?,
                weekday: row.get(2)?,
                start_time: row.get(3)?,
                end_time: row.get(4)?,
                period_label: row.get(5)?,
                is_active: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_weekday_aliases() {
        assert_eq!(weekday_to_index("mon").unwrap(), 0);
        assert_eq!(weekday_to_index("Friday").unwrap(), 4);
        assert!(weekday_to_index("funday").is_err());
    }
}
