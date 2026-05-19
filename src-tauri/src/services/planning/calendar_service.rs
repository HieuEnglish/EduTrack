use crate::services::hierarchy::{new_id, now, DbState};
use chrono::{Datelike, Days, NaiveDate};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AcademicCalendar {
    pub id: String,
    pub school_id: String,
    pub level_id: Option<String>,
    pub name: String,
    pub academic_year_start: String,
    pub academic_year_end: String,
    pub region_code: String,
    pub timezone: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CalendarStats {
    pub teachable_days: i32,
    pub holiday_days: i32,
    pub manual_closure_days: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CalendarClosureDay {
    pub id: String,
    pub calendar_id: String,
    pub closure_date: String,
    pub closure_type: String,
    pub title: Option<String>,
    pub source_provider: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicHoliday {
    date: String,
    local_name: Option<String>,
    name: Option<String>,
}

pub fn parse_date(value: &str, field: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|e| format!("invalid {field} '{value}': {e}"))
}

pub fn closure_dates(
    conn: &rusqlite::Connection,
    calendar_id: &str,
) -> Result<HashSet<NaiveDate>, String> {
    let mut stmt = conn
        .prepare("SELECT closure_date FROM calendar_closure_days WHERE calendar_id = ?1")
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map(params![calendar_id], |row| {
            let date: String = row.get(0)?;
            NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

pub fn teachable_dates_between(
    start: NaiveDate,
    end: NaiveDate,
    closures: &HashSet<NaiveDate>,
    weekdays: Option<&HashSet<u32>>,
) -> Vec<NaiveDate> {
    let mut dates = Vec::new();
    let mut current = start;
    while current <= end {
        let weekday = current.weekday().num_days_from_monday();
        let allowed = weekdays.map_or(weekday < 5, |days| days.contains(&weekday));
        if allowed && !closures.contains(&current) {
            dates.push(current);
        }
        current = current.checked_add_days(Days::new(1)).unwrap_or(current);
        if current == end.checked_add_days(Days::new(1)).unwrap_or(end) {
            break;
        }
    }
    dates
}

pub fn weekday_label(date: NaiveDate) -> String {
    match date.weekday().num_days_from_monday() {
        0 => "mon",
        1 => "tue",
        2 => "wed",
        3 => "thu",
        4 => "fri",
        5 => "sat",
        _ => "sun",
    }
    .to_string()
}

#[tauri::command]
pub async fn create_academic_calendar(
    state: DbState<'_>,
    mut calendar: AcademicCalendar,
) -> Result<String, String> {
    state.with_conn(|conn| {
        if calendar.id.is_empty() {
            calendar.id = new_id("calendar");
        }
        let timestamp = now();
        if calendar.created_at.is_empty() {
            calendar.created_at = timestamp.clone();
        }
        calendar.updated_at = timestamp;
        if calendar.timezone.is_empty() {
            calendar.timezone = "UTC".to_string();
        }
        if calendar.is_default {
            conn.execute(
                "UPDATE academic_calendars SET is_default = 0 WHERE school_id = ?1 AND COALESCE(level_id, '') = COALESCE(?2, '')",
                params![calendar.school_id, calendar.level_id],
            )
            .map_err(|e| e.to_string())?;
        }
        conn.execute(
            "INSERT INTO academic_calendars (id, school_id, level_id, name, academic_year_start, academic_year_end, region_code, timezone, is_default, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                calendar.id,
                calendar.school_id,
                calendar.level_id,
                calendar.name,
                calendar.academic_year_start,
                calendar.academic_year_end,
                calendar.region_code,
                calendar.timezone,
                calendar.is_default,
                calendar.created_at,
                calendar.updated_at
            ],
        )
        .map_err(|e| e.to_string())?;
        if calendar.is_default {
            if let Some(level_id) = &calendar.level_id {
                conn.execute(
                    "UPDATE levels SET default_calendar_id = ?1, updated_at = ?2 WHERE id = ?3",
                    params![calendar.id, now(), level_id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        Ok(calendar.id)
    })
}

#[tauri::command]
pub async fn add_manual_closure_day(
    state: DbState<'_>,
    calendar_id: String,
    date: String,
    reason: Option<String>,
) -> Result<String, String> {
    parse_date(&date, "closure date")?;
    state.with_conn(|conn| {
        let id = new_id("closure");
        conn.execute(
            "INSERT OR REPLACE INTO calendar_closure_days (id, calendar_id, closure_date, closure_type, title, source_provider, created_at)
             VALUES (?1, ?2, ?3, 'manual', ?4, 'manual', ?5)",
            params![id, calendar_id, date, reason, now()],
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    })
}

#[tauri::command]
pub async fn get_calendar_closure_days(
    state: DbState<'_>,
    calendar_id: String,
) -> Result<Vec<CalendarClosureDay>, String> {
    state.with_conn(|conn| load_closure_days(conn, &calendar_id))
}

#[tauri::command]
pub async fn sync_public_holidays(
    state: DbState<'_>,
    calendar_id: String,
    country_code: String,
    academic_year_start: String,
    academic_year_end: String,
) -> Result<Vec<CalendarClosureDay>, String> {
    let start = parse_date(&academic_year_start, "academic_year_start")?;
    let end = parse_date(&academic_year_end, "academic_year_end")?;
    if end < start {
        return Err("academic_year_end must be after academic_year_start".to_string());
    }

    let country_code = country_code.trim().to_ascii_uppercase();
    if country_code.len() != 2 || !country_code.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(
            "school region must be a two-letter country code before syncing holidays".to_string(),
        );
    }

    let fetch_country = country_code.clone();
    let holidays = tauri::async_runtime::spawn_blocking(move || {
        let mut holidays = Vec::new();
        for year in start.year()..=end.year() {
            holidays.extend(fetch_public_holidays(year, &fetch_country)?);
        }
        Ok::<Vec<PublicHoliday>, String>(holidays)
    })
    .await
    .map_err(|e| format!("holiday sync worker failed: {e}"))??;

    state.with_conn(|conn| {
        for holiday in holidays {
            let date = parse_date(&holiday.date, "holiday date")?;
            if date < start || date > end {
                continue;
            }
            let title = holiday
                .local_name
                .filter(|value| !value.trim().is_empty())
                .or(holiday.name)
                .unwrap_or_else(|| "Public holiday".to_string());
            conn.execute(
                "INSERT OR REPLACE INTO calendar_closure_days (id, calendar_id, closure_date, closure_type, title, source_provider, created_at)
                 VALUES (?1, ?2, ?3, 'holiday', ?4, 'nager.date', ?5)",
                params![new_id("closure"), calendar_id, date.format("%Y-%m-%d").to_string(), title, now()],
            )
            .map_err(|e| e.to_string())?;
        }
        load_closure_days(conn, &calendar_id)
    })
}

#[tauri::command]
pub async fn get_teachable_days(
    state: DbState<'_>,
    calendar_id: String,
    academic_year_start: String,
    academic_year_end: String,
) -> Result<i32, String> {
    state.with_conn(|conn| {
        let start = parse_date(&academic_year_start, "academic_year_start")?;
        let end = parse_date(&academic_year_end, "academic_year_end")?;
        let closures = closure_dates(conn, &calendar_id)?;
        Ok(teachable_dates_between(start, end, &closures, None).len() as i32)
    })
}

#[tauri::command]
pub async fn get_teachable_day_dates(
    state: DbState<'_>,
    calendar_id: String,
    academic_year_start: String,
    academic_year_end: String,
) -> Result<Vec<String>, String> {
    state.with_conn(|conn| {
        let start = parse_date(&academic_year_start, "academic_year_start")?;
        let end = parse_date(&academic_year_end, "academic_year_end")?;
        let closures = closure_dates(conn, &calendar_id)?;
        Ok(teachable_dates_between(start, end, &closures, None)
            .into_iter()
            .map(|d| d.format("%Y-%m-%d").to_string())
            .collect())
    })
}

#[tauri::command]
pub async fn get_calendar_stats(
    state: DbState<'_>,
    calendar_id: String,
    academic_year_start: String,
    academic_year_end: String,
) -> Result<CalendarStats, String> {
    state.with_conn(|conn| {
        let teachable_days =
            futures_free_teachable_days(conn, &calendar_id, &academic_year_start, &academic_year_end)?;
        let holiday_days: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar_closure_days WHERE calendar_id = ?1 AND closure_type = 'holiday'",
                params![calendar_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let manual_closure_days: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar_closure_days WHERE calendar_id = ?1 AND closure_type = 'manual'",
                params![calendar_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(CalendarStats {
            teachable_days,
            holiday_days,
            manual_closure_days,
        })
    })
}

fn futures_free_teachable_days(
    conn: &rusqlite::Connection,
    calendar_id: &str,
    academic_year_start: &str,
    academic_year_end: &str,
) -> Result<i32, String> {
    let start = parse_date(academic_year_start, "academic_year_start")?;
    let end = parse_date(academic_year_end, "academic_year_end")?;
    let closures = closure_dates(conn, calendar_id)?;
    Ok(teachable_dates_between(start, end, &closures, None).len() as i32)
}

fn fetch_public_holidays(year: i32, country_code: &str) -> Result<Vec<PublicHoliday>, String> {
    let url = format!(
        "https://date.nager.at/api/v3/PublicHolidays/{}/{}",
        year, country_code
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("EduTrack/0.1")
        .build()
        .map_err(|e| format!("could not create holiday client: {e}"))?;
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("could not fetch public holidays for {country_code} {year}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "holiday provider returned {} for {country_code} {year}",
            response.status()
        ));
    }
    response
        .json::<Vec<PublicHoliday>>()
        .map_err(|e| format!("could not read public holiday response: {e}"))
}

fn load_closure_days(
    conn: &rusqlite::Connection,
    calendar_id: &str,
) -> Result<Vec<CalendarClosureDay>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, calendar_id, closure_date, closure_type, title, source_provider, created_at
             FROM calendar_closure_days
             WHERE calendar_id = ?1
             ORDER BY closure_date, title",
        )
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map(params![calendar_id], |row| {
            Ok(CalendarClosureDay {
                id: row.get(0)?,
                calendar_id: row.get(1)?,
                closure_date: row.get(2)?,
                closure_type: row.get(3)?,
                title: row.get(4)?,
                source_provider: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_weekends_and_closures() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 1, 7).unwrap();
        let closures = [NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()]
            .into_iter()
            .collect();
        let dates = teachable_dates_between(start, end, &closures, None);
        let iso: Vec<_> = dates.iter().map(|d| d.to_string()).collect();
        assert_eq!(
            iso,
            vec!["2026-01-02", "2026-01-05", "2026-01-06", "2026-01-07"]
        );
    }

    #[test]
    fn all_weekdays_if_no_closures() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(); // Mon
        let end = NaiveDate::from_ymd_opt(2026, 1, 9).unwrap(); // Fri
        let closures = HashSet::new();
        let dates = teachable_dates_between(start, end, &closures, None);
        assert_eq!(dates.len(), 5);
    }

    #[test]
    fn custom_weekday_set() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(); // Mon
        let end = NaiveDate::from_ymd_opt(2026, 1, 11).unwrap(); // Sun
        let closures = HashSet::new();
        let mut weekdays = HashSet::new();
        weekdays.insert(0); // Mon only
        let dates = teachable_dates_between(start, end, &closures, Some(&weekdays));
        assert_eq!(dates.len(), 1);
        assert_eq!(dates[0].to_string(), "2026-01-05");
    }

    #[test]
    fn weekday_label_correct() {
        let d = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        assert_eq!(weekday_label(d), "mon");
        let d = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();
        assert_eq!(weekday_label(d), "tue");
        let d = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
        assert_eq!(weekday_label(d), "sat");
    }

    #[test]
    fn parse_date_valid() {
        let d = parse_date("2026-03-15", "test").unwrap();
        assert_eq!(d.year(), 2026);
        assert_eq!(d.month(), 3);
        assert_eq!(d.day(), 15);
    }

    #[test]
    fn parse_date_invalid() {
        assert!(parse_date("not-a-date", "test").is_err());
    }
}
