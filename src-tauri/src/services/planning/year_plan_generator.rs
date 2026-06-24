use crate::services::hierarchy::{
    get_class, get_level, new_id, now, CurriculumUnit, DbState, YearPlan, YearPlanLesson,
};
use crate::services::jobs::{
    complete_job, create_job, fail_job, is_cancel_requested, mark_job_running, update_job_progress,
};
use crate::services::llm_service::{generate_local_report, load_llm_config};
use crate::services::planning::calendar_service::{
    closure_dates, parse_date, teachable_dates_between, weekday_label,
};
use crate::services::planning::scheduling::class_weekday_set;
use crate::services::syllabus_processing::load_units;
use chrono::Datelike;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

struct LessonPlanPreload {
    lesson_id: String,
    unit_title: String,
    unit_description: Option<String>,
    unit_assessment: Option<String>,
    lesson_title: String,
    teaching_date: String,
    part_index: usize,
    total_parts: usize,
    is_buffer: bool,
    has_existing_plan: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeneratePlanOptions {
    pub level_id: String,
    pub class_id: Option<String>,
    pub syllabus_id: String,
    pub calendar_id: String,
    pub buffer_days_percent: Option<f64>,
    pub pacing_mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct YearPlanBundle {
    pub year_plan: YearPlan,
    pub lessons: Vec<YearPlanLesson>,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct DetailedPlanGenerationGateState {
    in_flight: bool,
    last_started_at: Option<Instant>,
}

static DETAILED_PLAN_GENERATION_GATE: OnceLock<Mutex<DetailedPlanGenerationGateState>> =
    OnceLock::new();

struct DetailedPlanGenerationPermit<'a> {
    gate: &'a Mutex<DetailedPlanGenerationGateState>,
}

impl Drop for DetailedPlanGenerationPermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.gate.lock() {
            state.in_flight = false;
        }
    }
}

fn generation_gate() -> &'static Mutex<DetailedPlanGenerationGateState> {
    DETAILED_PLAN_GENERATION_GATE
        .get_or_init(|| Mutex::new(DetailedPlanGenerationGateState::default()))
}

fn acquire_detailed_plan_generation_permit(
    llm_config: &crate::services::llm_service::LocalModelConfig,
) -> Result<DetailedPlanGenerationPermit<'static>, String> {
    let gate = generation_gate();
    let cooldown = cloud_generation_cooldown(llm_config);

    loop {
        let wait_for = {
            let mut state = gate
                .lock()
                .map_err(|_| "AI generation gate lock poisoned".to_string())?;
            if state.in_flight {
                Some(Duration::from_millis(250))
            } else if let Some(last_started_at) = state.last_started_at {
                let elapsed = last_started_at.elapsed();
                if elapsed < cooldown {
                    Some(cooldown - elapsed)
                } else {
                    state.in_flight = true;
                    state.last_started_at = Some(Instant::now());
                    None
                }
            } else {
                state.in_flight = true;
                state.last_started_at = Some(Instant::now());
                None
            }
        };

        if let Some(delay) = wait_for {
            std::thread::sleep(delay);
            continue;
        }

        return Ok(DetailedPlanGenerationPermit { gate });
    }
}

fn cloud_generation_cooldown(
    llm_config: &crate::services::llm_service::LocalModelConfig,
) -> Duration {
    if !is_cloud_backed_model(llm_config) {
        return Duration::from_millis(0);
    }
    let configured = std::env::var("EDUTRACK_CLOUD_REQUEST_COOLDOWN_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(12_000);
    Duration::from_millis(configured.clamp(1_000, 120_000))
}

fn is_cloud_backed_model(llm_config: &crate::services::llm_service::LocalModelConfig) -> bool {
    if llm_config.provider == "opencode" {
        return true;
    }
    let model = llm_config.model.to_ascii_lowercase();
    model.contains(":cloud") || model.contains("-cloud")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CalendarLessonView {
    pub lesson: YearPlanLesson,
    pub class_id: String,
    pub class_name: String,
    pub level_name: String,
    pub school_name: String,
    pub subject_name: Option<String>,
}

fn normalize_unit_title(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_unit_description(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .and_then(|value| if value.is_empty() { None } else { Some(value) })
}

fn duplicated_unit_titles(units: &[CurriculumUnit]) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut originals: HashMap<String, String> = HashMap::new();
    for unit in units {
        let key = unit.title.to_ascii_lowercase();
        *counts.entry(key.clone()).or_insert(0) += 1;
        originals.entry(key).or_insert_with(|| unit.title.clone());
    }
    let mut duplicates: Vec<String> = counts
        .into_iter()
        .filter_map(|(key, count)| {
            if count > 1 {
                originals.get(&key).cloned()
            } else {
                None
            }
        })
        .collect();
    duplicates.sort();
    duplicates
}

fn requested_lessons_for_unit(unit: &CurriculumUnit, meetings_per_week: usize) -> usize {
    let lesson_based = unit.estimated_lessons.max(1) as usize;
    let weeks = unit.estimated_weeks.max(0) as usize;
    let week_based = if weeks > 0 && weeks != lesson_based {
        weeks.saturating_mul(meetings_per_week.max(1))
    } else {
        0
    };
    lesson_based.max(week_based).min(240)
}

fn distribute_remaining_slots(
    indices: &[usize],
    requested: &[usize],
    allocated: &mut [usize],
    remaining: &mut usize,
) {
    if *remaining == 0 || indices.is_empty() {
        return;
    }
    let extras: Vec<usize> = indices
        .iter()
        .map(|idx| requested[*idx].saturating_sub(allocated[*idx]))
        .collect();
    let extras_total: usize = extras.iter().sum();
    if extras_total == 0 {
        return;
    }

    let distributable = *remaining;
    let mut remainders: Vec<(usize, f64)> = Vec::new();
    for (pos, idx) in indices.iter().enumerate() {
        let extra = extras[pos];
        if extra == 0 {
            continue;
        }
        let exact = (extra as f64) * (distributable as f64) / (extras_total as f64);
        let floor = exact.floor() as usize;
        let take = floor.min(extra);
        allocated[*idx] += take;
        *remaining = remaining.saturating_sub(take);
        remainders.push((*idx, exact - floor as f64));
    }

    if *remaining > 0 {
        remainders.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        for (idx, _) in remainders {
            if *remaining == 0 {
                break;
            }
            if allocated[idx] < requested[idx] {
                allocated[idx] += 1;
                *remaining -= 1;
            }
        }
    }
}

fn allocate_lessons_per_unit(
    units: &[CurriculumUnit],
    requested: &[usize],
    instruction_slots: usize,
) -> (Vec<usize>, usize, usize) {
    if units.is_empty() || instruction_slots == 0 {
        let required_dropped = units.iter().filter(|unit| unit.is_required).count();
        let optional_dropped = units.len().saturating_sub(required_dropped);
        return (vec![0; units.len()], required_dropped, optional_dropped);
    }

    let mut allocated = vec![0usize; units.len()];
    let required_indices: Vec<usize> = units
        .iter()
        .enumerate()
        .filter(|(_, unit)| unit.is_required)
        .map(|(idx, _)| idx)
        .collect();
    let optional_indices: Vec<usize> = units
        .iter()
        .enumerate()
        .filter(|(_, unit)| !unit.is_required)
        .map(|(idx, _)| idx)
        .collect();

    let mut remaining = instruction_slots;

    for idx in required_indices.iter().chain(optional_indices.iter()) {
        if remaining == 0 {
            break;
        }
        if requested[*idx] > 0 {
            allocated[*idx] = 1;
            remaining -= 1;
        }
    }

    distribute_remaining_slots(&required_indices, requested, &mut allocated, &mut remaining);
    distribute_remaining_slots(&optional_indices, requested, &mut allocated, &mut remaining);

    if remaining > 0 {
        let all_indices: Vec<usize> = (0..units.len()).collect();
        distribute_remaining_slots(&all_indices, requested, &mut allocated, &mut remaining);
    }

    let required_dropped = required_indices
        .iter()
        .filter(|idx| allocated[**idx] == 0)
        .count();
    let optional_dropped = optional_indices
        .iter()
        .filter(|idx| allocated[**idx] == 0)
        .count();
    (allocated, required_dropped, optional_dropped)
}

#[tauri::command]
pub async fn get_all_calendar_lessons(
    state: DbState<'_>,
) -> Result<Vec<CalendarLessonView>, String> {
    state.with_conn(|conn| {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        conn.execute(
            "UPDATE year_plan_lessons SET status = 'missed', updated_at = ?1 WHERE status = 'planned' AND teaching_date < ?2 AND is_buffer = 0",
            params![now(), today],
        )
        .map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT l.id, l.year_plan_id, l.teaching_date, l.weekday, l.sequence_number,
                        l.curriculum_unit_id, l.lesson_title, l.lesson_objective, l.detailed_plan_attached, l.coverage_weight,
                        l.is_buffer, l.is_holiday_adjusted, l.status, l.created_at, l.updated_at,
                        c.id, c.name, c.subject_name, lev.name, s.name
                 FROM year_plan_lessons l
                 JOIN year_plans p ON l.year_plan_id = p.id
                 JOIN classes c ON p.class_id = c.id
                 JOIN levels lev ON c.level_id = lev.id
                 JOIN schools s ON c.school_id = s.id
                 WHERE c.archived_at IS NULL AND s.archived_at IS NULL
                 ORDER BY l.teaching_date, s.name, lev.display_order, c.name",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_map([], |row| {
                Ok(CalendarLessonView {
                    lesson: YearPlanLesson {
                        id: row.get(0)?,
                        year_plan_id: row.get(1)?,
                        teaching_date: row.get(2)?,
                        weekday: row.get(3)?,
                        sequence_number: row.get(4)?,
                        curriculum_unit_id: row.get(5)?,
                        lesson_title: row.get(6)?,
                        lesson_objective: row.get(7)?,
                        detailed_plan_attached: row.get(8)?,
                        coverage_weight: row.get(9)?,
                        is_buffer: row.get(10)?,
                        is_holiday_adjusted: row.get(11)?,
                        status: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                    },
                    class_id: row.get(15)?,
                    class_name: row.get(16)?,
                    subject_name: row.get(17)?,
                    level_name: row.get(18)?,
                    school_name: row.get(19)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string());
        result
    })
}

#[tauri::command]
pub async fn generate_year_plan(
    state: DbState<'_>,
    options: GeneratePlanOptions,
) -> Result<YearPlanBundle, String> {
    state.with_conn(|conn| {
        let level = get_level(conn, &options.level_id)?;
        let mut warnings = Vec::new();
        let mut calendar_id = options.calendar_id.clone();
        let calendar_exists: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM academic_calendars WHERE id = ?1 AND school_id = ?2)",
                params![calendar_id, level.school_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if calendar_exists == 0 {
            let fallback_default = level.default_calendar_id.clone().filter(|id| !id.is_empty());
            if let Some(default_calendar_id) = fallback_default {
                let default_exists: i64 = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM academic_calendars WHERE id = ?1 AND school_id = ?2)",
                        params![default_calendar_id, level.school_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                if default_exists != 0 {
                    calendar_id = default_calendar_id;
                    warnings.push("Selected calendar was missing; using the level default calendar instead.".to_string());
                }
            }
            if calendar_id == options.calendar_id {
                calendar_id = new_id("calendar");
                let timestamp = now();
                conn.execute(
                    "UPDATE academic_calendars SET is_default = 0 WHERE school_id = ?1 AND COALESCE(level_id, '') = COALESCE(?2, '')",
                    params![level.school_id, level.id],
                )
                .map_err(|e| e.to_string())?;
                conn.execute(
                    "INSERT INTO academic_calendars (id, school_id, level_id, name, academic_year_start, academic_year_end, region_code, timezone, is_default, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10)",
                    params![
                        calendar_id,
                        level.school_id,
                        level.id,
                        format!("{} Calendar", level.name),
                        level.academic_year_start,
                        level.academic_year_end,
                        level.region_code,
                        level.timezone,
                        timestamp,
                        timestamp
                    ],
                )
                .map_err(|e| e.to_string())?;
                conn.execute(
                    "UPDATE levels SET default_calendar_id = ?1, updated_at = ?2 WHERE id = ?3",
                    params![calendar_id, now(), level.id],
                )
                .map_err(|e| e.to_string())?;
                warnings.push("Detected a stale calendar reference and rebuilt the level calendar. Re-sync public holidays if needed.".to_string());
            }
        }
        let units = load_units(conn, &options.syllabus_id)?;
        if units.is_empty() {
            return Err("no reviewed curriculum units found for this syllabus".to_string());
        }
        let units: Vec<CurriculumUnit> = units
            .into_iter()
            .map(|mut unit| {
                unit.title = normalize_unit_title(&unit.title);
                unit.description = normalize_unit_description(unit.description);
                unit
            })
            .collect();
        if let Some((idx, _)) = units
            .iter()
            .enumerate()
            .find(|(_, unit)| unit.title.is_empty())
        {
            return Err(format!(
                "syllabus unit #{} has an empty title; review extracted units before generating a schedule",
                idx + 1
            ));
        }
        if let Some((idx, unit)) = units
            .iter()
            .enumerate()
            .find(|(_, unit)| unit.estimated_lessons <= 0 || unit.estimated_weeks < 0)
        {
            return Err(format!(
                "syllabus unit #{} ('{}') has invalid lesson/week estimates; set lessons >= 1 and weeks >= 0",
                idx + 1,
                unit.title
            ));
        }
        let duplicate_titles = duplicated_unit_titles(&units);
        if !duplicate_titles.is_empty() {
            warnings.push(format!(
                "Duplicate syllabus unit titles detected: {}. Scheduling continued using unit IDs; rename duplicates if you want clearer lesson labels.",
                duplicate_titles.join(", ")
            ));
        }

        let class = options
            .class_id
            .as_deref()
            .map(|class_id| get_class(conn, class_id))
            .transpose()?;
        let start_value = class
            .as_ref()
            .and_then(|item| item.start_date.as_ref())
            .unwrap_or(&level.academic_year_start);
        let end_value = class
            .as_ref()
            .and_then(|item| item.end_date.as_ref())
            .unwrap_or(&level.academic_year_end);
        let start = parse_date(start_value, "class_start_date")?;
        let end = parse_date(end_value, "class_end_date")?;
        if end < start {
            return Err("class end date is earlier than start date".to_string());
        }
        let closures = closure_dates(conn, &calendar_id)?;
        let weekdays = class_weekday_set(conn, options.class_id.as_deref())?;
        if options.class_id.is_some() && weekdays.is_none() {
            return Err("no active class meeting days found. Set class schedule rules before generating lesson schedules.".to_string());
        }
        let teachable_dates = teachable_dates_between(start, end, &closures, weekdays.as_ref());
        if teachable_dates.is_empty() {
            return Err("no teachable dates are available for this calendar and schedule".to_string());
        }
        let closures_in_year = closures
            .iter()
            .filter(|date| **date >= start && **date <= end)
            .count();
        let class_closures_skipped = closures
            .iter()
            .filter(|date| {
                **date >= start
                    && **date <= end
                    && weekdays.as_ref().map_or(date.weekday().num_days_from_monday() < 5, |days| {
                        days.contains(&date.weekday().num_days_from_monday())
                    })
            })
            .count();

        let meetings_per_week = weekdays
            .as_ref()
            .map(|days| days.len().max(1))
            .unwrap_or(5);
        let requested_per_unit: Vec<usize> = units
            .iter()
            .map(|unit| requested_lessons_for_unit(unit, meetings_per_week))
            .collect();
        let requested_lessons: usize = requested_per_unit.iter().sum();
        let buffer_percent = options.buffer_days_percent.unwrap_or(0.08).clamp(0.0, 0.25);
        let buffer_days = ((teachable_dates.len() as f64) * buffer_percent).round() as usize;
        let instruction_slots = teachable_dates.len().saturating_sub(buffer_days).max(1);
        if requested_lessons > instruction_slots {
            warnings.push(format!(
                "Syllabus requests {requested_lessons} lessons but only {instruction_slots} instructional days are available after reserving {buffer_days} buffer day(s). Lesson counts have been proportionally scaled to fit."
            ));
        }
        if class_closures_skipped > 0 {
            warnings.push(format!(
                "{class_closures_skipped} scheduled class days were skipped for public holidays or closure days."
            ));
        }

        let (allocated_per_unit, required_dropped, optional_dropped) =
            allocate_lessons_per_unit(&units, &requested_per_unit, instruction_slots);
        let allocated_total: usize = allocated_per_unit.iter().sum();
        if required_dropped > 0 {
            warnings.push(format!(
                "{required_dropped} required unit(s) could not be scheduled. Review class calendar capacity and syllabus unit estimates."
            ));
        }
        if optional_dropped > 0 {
            warnings.push(format!(
                "{optional_dropped} optional unit(s) were dropped to fit available instruction days."
            ));
        }
        if allocated_total != requested_lessons && allocated_total > 0 {
            warnings.push(format!(
                "Lesson count adjusted from {requested_lessons} requested to {allocated_total} scheduled to fit available calendar days."
            ));
        }
        let dropped_units = required_dropped + optional_dropped;

        let mut lesson_specs = Vec::new();
        for (unit, allocated_count) in units.iter().zip(allocated_per_unit.iter()) {
            let lesson_count = *allocated_count;
            if lesson_count == 0 {
                continue;
            }
            let sub_topics = generate_sub_topics(&unit.description, lesson_count);
            for part in 1..=lesson_count {
                let sub = sub_topics.get(part - 1).filter(|t| !t.is_empty());
                let title = if lesson_count > 1 {
                    if let Some(topic) = sub {
                        format!("{}: {}", unit.title, topic)
                    } else {
                        format!("{} (Part {}/{})", unit.title, part, lesson_count)
                    }
                } else {
                    unit.title.clone()
                };
                let lesson_desc = sub.map(|s| s.to_string()).or_else(|| unit.description.clone());
                let objective = generate_schedule_note(&unit.title, &lesson_desc, part, lesson_count);
                lesson_specs.push((
                    Some(unit.id.clone()),
                    title,
                    Some(objective),
                    1.0 / lesson_count as f64,
                ));
            }
        }

        let ordered_dates = pace_dates(&teachable_dates, options.pacing_mode.as_deref());
        let plan_id = new_id("plan");
        let timestamp = now();
        let mut lessons = Vec::new();
        let non_buffer_total = lesson_specs.len().min(instruction_slots);

        for (index, date) in ordered_dates.iter().enumerate().take(non_buffer_total) {
            let sequence = (index + 1) as i32;
            let (unit_id, title, objective, weight) = lesson_specs.get(index).unwrap();
            lessons.push(YearPlanLesson {
                id: new_id("lesson"),
                year_plan_id: plan_id.clone(),
                teaching_date: date.format("%Y-%m-%d").to_string(),
                weekday: weekday_label(*date),
                sequence_number: sequence,
                curriculum_unit_id: unit_id.clone(),
                lesson_title: title.clone(),
                lesson_objective: objective.clone(),
                detailed_plan_attached: false,
                coverage_weight: Some(*weight),
                is_buffer: false,
                is_holiday_adjusted: false,
                status: "planned".to_string(),
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
            });
        }

        for (offset, date) in ordered_dates.iter().skip(non_buffer_total).enumerate() {
            let sequence = (non_buffer_total + offset + 1) as i32;
            lessons.push(YearPlanLesson {
                id: new_id("lesson"),
                year_plan_id: plan_id.clone(),
                teaching_date: date.format("%Y-%m-%d").to_string(),
                weekday: weekday_label(*date),
                sequence_number: sequence,
                curriculum_unit_id: None,
                lesson_title: "Buffer / reteaching day".to_string(),
                lesson_objective: Some("Review, reassess, or absorb calendar drift".to_string()),
                detailed_plan_attached: false,
                coverage_weight: None,
                is_buffer: true,
                is_holiday_adjusted: false,
                status: "planned".to_string(),
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
            });
        }

        let period_minutes = class.as_ref().map(|c| c.period_minutes).unwrap_or(45);
        let snapshot = serde_json::json!({
            "unitCount": units.len(),
            "requestedLessons": requested_lessons,
            "allocatedInstructionLessons": allocated_total,
            "unscheduledUnits": dropped_units,
            "pacingMode": options.pacing_mode.as_deref().unwrap_or("balanced"),
            "classScheduleApplied": weekdays.is_some(),
            "closureDaysInYear": closures_in_year,
            "scheduledClosureDaysSkipped": class_closures_skipped,
            "periodMinutes": period_minutes,
            "warnings": warnings,
        })
        .to_string();

        let plan = YearPlan {
            id: plan_id.clone(),
            school_id: level.school_id.clone(),
            level_id: level.id.clone(),
            class_id: options.class_id.clone(),
            syllabus_id: options.syllabus_id.clone(),
            calendar_id: calendar_id.clone(),
            region_code: level.region_code.clone(),
            plan_scope: if options.class_id.is_some() {
                "class_specific".to_string()
            } else {
                "level_default".to_string()
            },
            generation_status: "generated".to_string(),
            generation_summary: Some(format!(
                "Generated {} instructional lessons (+{} buffer days) from {} curriculum units. {} minutes per lesson.",
                lessons.iter().filter(|lesson| !lesson.is_buffer).count(),
                lessons.iter().filter(|lesson| lesson.is_buffer).count(),
                units.len(),
                period_minutes,
            )),
            total_teaching_days: teachable_dates.len() as i32,
            total_planned_lessons: lessons.iter().filter(|lesson| !lesson.is_buffer).count() as i32,
            holiday_days_excluded: closures_in_year as i32,
            buffer_days_reserved: lessons.iter().filter(|lesson| lesson.is_buffer).count() as i32,
            plan_snapshot_json: snapshot,
            generated_at: Some(timestamp.clone()),
            published_at: None,
            superseded_by_plan_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };

        conn.execute(
            "INSERT INTO year_plans (id, school_id, level_id, class_id, syllabus_id, calendar_id, region_code, plan_scope, generation_status, generation_summary, total_teaching_days, total_planned_lessons, holiday_days_excluded, buffer_days_reserved, plan_snapshot_json, generated_at, published_at, superseded_by_plan_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                plan.id,
                plan.school_id,
                plan.level_id,
                plan.class_id,
                plan.syllabus_id,
                plan.calendar_id,
                plan.region_code,
                plan.plan_scope,
                plan.generation_status,
                plan.generation_summary,
                plan.total_teaching_days,
                plan.total_planned_lessons,
                plan.holiday_days_excluded,
                plan.buffer_days_reserved,
                plan.plan_snapshot_json,
                plan.generated_at,
                plan.published_at,
                plan.superseded_by_plan_id,
                plan.created_at,
                plan.updated_at
            ],
        )
        .map_err(|e| e.to_string())?;

        for lesson in &lessons {
            conn.execute(
                "INSERT INTO year_plan_lessons (id, year_plan_id, teaching_date, weekday, sequence_number, curriculum_unit_id, lesson_title, lesson_objective, detailed_plan_attached, coverage_weight, is_buffer, is_holiday_adjusted, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    lesson.id,
                    lesson.year_plan_id,
                    lesson.teaching_date,
                    lesson.weekday,
                    lesson.sequence_number,
                    lesson.curriculum_unit_id,
                    lesson.lesson_title,
                    lesson.lesson_objective,
                    lesson.detailed_plan_attached,
                    lesson.coverage_weight,
                    lesson.is_buffer,
                    lesson.is_holiday_adjusted,
                    lesson.status,
                    lesson.created_at,
                    lesson.updated_at
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        if let Some(class_id) = &options.class_id {
            conn.execute(
                "UPDATE classes SET year_plan_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![plan_id, now(), class_id],
            )
            .map_err(|e| e.to_string())?;
        }

        Ok(YearPlanBundle {
            year_plan: plan,
            lessons,
            warnings,
        })
    })
}

fn generate_sub_topics(description: &Option<String>, count: usize) -> Vec<String> {
    if count <= 1 {
        return vec![];
    }
    if let Some(desc) = description {
        let mut parts: Vec<String> = Vec::new();
        for line in desc.lines() {
            let cleaned = line
                .trim()
                .trim_start_matches(|c: char| {
                    c == '-' || c == '*' || c == '\u{2022}' || c == '\u{2013}' || c == '\u{2014}'
                })
                .trim();
            if cleaned.len() >= 8 {
                parts.push(cleaned.to_string());
            }
        }
        if parts.len() < count {
            for sentence in desc
                .split(|c: char| c == '.' || c == '!' || c == '?' || c == ';')
                .map(str::trim)
                .filter(|s| s.len() >= 15)
            {
                if !parts.iter().any(|p| p.eq_ignore_ascii_case(sentence)) {
                    parts.push(sentence.to_string());
                }
            }
        }
        if parts.is_empty() {
            return (1..=count)
                .map(|i| format!("Core skill practice {i}"))
                .collect();
        }
        if parts.len() >= count {
            return parts
                .into_iter()
                .take(count)
                .map(|topic| topic.chars().take(90).collect::<String>())
                .collect();
        }
        return (0..count)
            .map(|idx| {
                let base = parts[idx % parts.len()].clone();
                if parts.len() == 1 {
                    format!("{base} (focus {})", idx + 1)
                } else {
                    base
                }
            })
            .collect();
    }
    (1..=count)
        .map(|i| format!("Lesson {i} of {count}"))
        .collect()
}

fn generate_schedule_note(
    unit_title: &str,
    description: &Option<String>,
    part: usize,
    total: usize,
) -> String {
    let focus = description
        .as_deref()
        .map(|value| value.chars().take(120).collect::<String>())
        .unwrap_or_else(|| unit_title.to_string());
    if total <= 1 {
        return format!("Planned focus: {focus}");
    }
    let stage = if part == 1 {
        "Launch"
    } else if part == total {
        "Mastery and assessment"
    } else {
        "Development"
    };
    format!("{stage} ({part}/{total}) - {focus}")
}

fn pace_dates(dates: &[chrono::NaiveDate], pacing_mode: Option<&str>) -> Vec<chrono::NaiveDate> {
    let mut result = dates.to_vec();
    match pacing_mode.unwrap_or("balanced") {
        "front_loaded" => result,
        "back_loaded" => {
            let shift = result.len() / 10;
            if shift > 0 {
                result.rotate_left(shift);
            }
            result
        }
        _ => result,
    }
}

#[tauri::command]
pub async fn get_year_plan(
    state: DbState<'_>,
    year_plan_id: String,
) -> Result<YearPlanBundle, String> {
    state.with_conn(|conn| {
        let plan = conn
            .query_row(
                "SELECT id, school_id, level_id, class_id, syllabus_id, calendar_id, region_code, plan_scope, generation_status, generation_summary, total_teaching_days, total_planned_lessons, holiday_days_excluded, buffer_days_reserved, plan_snapshot_json, generated_at, published_at, superseded_by_plan_id, created_at, updated_at FROM year_plans WHERE id = ?1",
                params![year_plan_id],
                |row| {
                    Ok(YearPlan {
                        id: row.get(0)?,
                        school_id: row.get(1)?,
                        level_id: row.get(2)?,
                        class_id: row.get(3)?,
                        syllabus_id: row.get(4)?,
                        calendar_id: row.get(5)?,
                        region_code: row.get(6)?,
                        plan_scope: row.get(7)?,
                        generation_status: row.get(8)?,
                        generation_summary: row.get(9)?,
                        total_teaching_days: row.get(10)?,
                        total_planned_lessons: row.get(11)?,
                        holiday_days_excluded: row.get(12)?,
                        buffer_days_reserved: row.get(13)?,
                        plan_snapshot_json: row.get(14)?,
                        generated_at: row.get(15)?,
                        published_at: row.get(16)?,
                        superseded_by_plan_id: row.get(17)?,
                        created_at: row.get(18)?,
                        updated_at: row.get(19)?,
                    })
                },
            )
            .map_err(|e| e.to_string())?;
        let lessons = load_lessons(conn, &plan.id)?;
        Ok(YearPlanBundle {
            year_plan: plan,
            lessons,
            warnings: Vec::new(),
        })
    })
}

#[tauri::command]
pub async fn publish_year_plan(
    state: DbState<'_>,
    year_plan_id: String,
) -> Result<YearPlan, String> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE year_plans SET generation_status = 'published', published_at = ?1, updated_at = ?1 WHERE id = ?2 AND published_at IS NULL",
            params![now(), year_plan_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn
            .query_row(
                "SELECT id, school_id, level_id, class_id, syllabus_id, calendar_id, region_code, plan_scope, generation_status, generation_summary, total_teaching_days, total_planned_lessons, holiday_days_excluded, buffer_days_reserved, plan_snapshot_json, generated_at, published_at, superseded_by_plan_id, created_at, updated_at FROM year_plans WHERE id = ?1",
                params![year_plan_id],
                |row| {
                    Ok(YearPlan {
                        id: row.get(0)?,
                        school_id: row.get(1)?,
                        level_id: row.get(2)?,
                        class_id: row.get(3)?,
                        syllabus_id: row.get(4)?,
                        calendar_id: row.get(5)?,
                        region_code: row.get(6)?,
                        plan_scope: row.get(7)?,
                        generation_status: row.get(8)?,
                        generation_summary: row.get(9)?,
                        total_teaching_days: row.get(10)?,
                        total_planned_lessons: row.get(11)?,
                        holiday_days_excluded: row.get(12)?,
                        buffer_days_reserved: row.get(13)?,
                        plan_snapshot_json: row.get(14)?,
                        generated_at: row.get(15)?,
                        published_at: row.get(16)?,
                        superseded_by_plan_id: row.get(17)?,
                        created_at: row.get(18)?,
                        updated_at: row.get(19)?,
                    })
                },
            )
            .map_err(|e| e.to_string())?)
    })
}

#[tauri::command]
pub async fn get_year_plan_lessons(
    state: DbState<'_>,
    year_plan_id: String,
) -> Result<Vec<YearPlanLesson>, String> {
    state.with_conn(|conn| load_lessons(conn, &year_plan_id))
}

#[tauri::command]
pub async fn generate_detailed_lesson_plans(
    state: DbState<'_>,
    class_id: String,
    context_notes: Option<String>,
    template_content: Option<String>,
    regenerate_existing: Option<bool>,
    use_llm: Option<bool>,
) -> Result<Vec<YearPlanLesson>, String> {
    let regenerate = regenerate_existing.unwrap_or(false);
    let context = context_notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|s| s.to_string());
    let template = template_content
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|s| s.to_string());
    if !use_llm.unwrap_or(true) {
        return Err("AI detailed planning is required and cannot be disabled.".to_string());
    }

    let (class, llm_config, plan_id, preloads, job_id) = {
        state.with_conn(|conn| {
            let class = get_class(conn, &class_id).map_err(|e| e.to_string())?;
            let llm_config = load_llm_config(conn);
            let plan_id = class
                .year_plan_id
                .clone()
                .ok_or_else(|| "No year plan linked to this class".to_string())?;
            let lessons = load_lessons(conn, &plan_id)?;
            let preloads = build_lesson_preloads(conn, &lessons, &plan_id, regenerate)?;
            let total = preloads
                .iter()
                .filter(|preload| !preload.is_buffer && (regenerate || !preload.has_existing_plan))
                .count() as i32;
            let job_id = create_job(
                conn,
                "lesson_plan_generation",
                Some("class"),
                Some(&class_id),
                total,
                Some("Queued detailed lesson plan generation"),
            )?;
            mark_job_running(conn, &job_id, Some("Generating detailed lesson plans"))?;
            Ok::<_, String>((class, llm_config, plan_id, preloads, job_id))
        })?
    };

    let mut results: Vec<(String, String)> = Vec::new();
    let total = preloads
        .iter()
        .filter(|preload| !preload.is_buffer && (regenerate || !preload.has_existing_plan))
        .count() as i32;
    for preload in preloads {
        if preload.is_buffer || (preload.has_existing_plan && !regenerate) {
            continue;
        }
        if state.with_conn(|conn| is_cancel_requested(conn, &job_id))? {
            state.with_conn(|conn| fail_job(conn, &job_id, "lesson plan generation cancelled"))?;
            return Err("lesson plan generation cancelled".to_string());
        }
        let _permit = acquire_detailed_plan_generation_permit(&llm_config)?;
        let description = preload
            .lesson_title
            .split(':')
            .nth(1)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| preload.unit_description.clone());
        let objective = generate_lesson_objective_with_llm(
            &llm_config,
            &class.name,
            class.subject_name.as_deref(),
            &preload.unit_title,
            &description,
            &preload.unit_assessment,
            &preload.lesson_title,
            &preload.teaching_date,
            preload.part_index,
            preload.total_parts,
            class.period_minutes,
            context.as_deref(),
            template.as_deref(),
        )
        .map_err(|err| {
            let message = format!(
                "AI lesson-plan generation failed for '{}': {}",
                preload.lesson_title,
                sanitize_provider_error(&err)
            );
            let _ = state.with_conn(|conn| fail_job(conn, &job_id, &message));
            message
        })?;
        results.push((preload.lesson_id.clone(), objective));
        state.with_conn(|conn| {
            update_job_progress(
                conn,
                &job_id,
                results.len() as i32,
                total,
                Some(&format!("Generated {}", preload.lesson_title)),
            )
        })?;
    }

    let timestamp = now();
    state.with_conn(|conn| {
        for (lesson_id, objective) in &results {
            let is_detailed = serde_json::from_str::<serde_json::Value>(objective)
                .map(|v| v.get("v").and_then(|v| v.as_str()) == Some("1"))
                .unwrap_or(false);
            conn.execute(
                "UPDATE year_plan_lessons SET lesson_objective = ?1, detailed_plan_attached = ?2, updated_at = ?3 WHERE id = ?4",
                params![objective, is_detailed, timestamp, lesson_id],
            )
            .map_err(|e| e.to_string())?;
        }
        complete_job(conn, &job_id, Some("Detailed lesson plan generation finished"))?;
        load_lessons(conn, &plan_id)
    })
}

#[tauri::command]
pub async fn generate_detailed_lesson_plan_for_lesson(
    state: DbState<'_>,
    class_id: String,
    lesson_id: String,
    context_notes: Option<String>,
    template_content: Option<String>,
    generate_for_flex_day_activities: Option<bool>,
) -> Result<YearPlanLesson, String> {
    let context = context_notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|s| s.to_string());
    let template = template_content
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|s| s.to_string());
    let flex_mode = generate_for_flex_day_activities.unwrap_or(false);

    let (class, llm_config, lesson, job_id) = {
        state.with_conn(|conn| {
            let class = get_class(conn, &class_id).map_err(|e| e.to_string())?;
            let llm_config = load_llm_config(conn);
            let plan_id = class
                .year_plan_id
                .clone()
                .ok_or_else(|| "No year plan linked to this class".to_string())?;
            let lessons = load_lessons(conn, &plan_id)?;
            let lesson = lessons
                .into_iter()
                .find(|l| l.id == lesson_id)
                .ok_or_else(|| "Lesson not found".to_string())?;
            let job_id = create_job(
                conn,
                "lesson_plan_generation",
                Some("lesson"),
                Some(&lesson_id),
                1,
                Some("Queued detailed lesson plan generation"),
            )?;
            mark_job_running(conn, &job_id, Some("Generating detailed lesson plan"))?;
            Ok::<_, String>((class, llm_config, lesson, job_id))
        })?
    };

    if lesson.is_buffer && !flex_mode {
        return Err("Buffer lessons do not generate detailed lesson plans.".to_string());
    }

    let (unit_title, unit_description, unit_assessment) = {
        if lesson.is_buffer {
            (
                "Flex Day Classroom Activities".to_string(),
                Some(
                    "Generate a full-lesson activity plan tied to current syllabus goals and yearly pacing. This day remains a flex/catch-up slot and can be replaced by rescheduled missed lessons."
                        .to_string(),
                ),
                Some("Use quick formative checks, teacher observation, and student reflection during activities.".to_string()),
            )
        } else {
            state.with_conn(|conn| {
                let unit_id = lesson.curriculum_unit_id.clone()
                    .ok_or_else(|| format!("lesson '{}' is missing syllabus unit mapping", lesson.lesson_title))?;
                conn.query_row(
                    "SELECT title, description, assessment_hint FROM curriculum_units WHERE id = ?1",
                    params![unit_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|_| "could not load syllabus unit details".to_string())
            })?
        }
    };

    let _permit = acquire_detailed_plan_generation_permit(&llm_config)?;
    let objective = generate_lesson_objective_with_llm(
        &llm_config,
        &class.name,
        class.subject_name.as_deref(),
        &unit_title,
        &unit_description,
        &unit_assessment,
        &lesson.lesson_title,
        &lesson.teaching_date,
        1,
        1,
        class.period_minutes,
        context.as_deref(),
        template.as_deref(),
    )
    .map_err(|err| {
        let message = format!(
            "AI lesson-plan generation failed for '{}': {}",
            lesson.lesson_title,
            sanitize_provider_error(&err)
        );
        let _ = state.with_conn(|conn| fail_job(conn, &job_id, &message));
        message
    })?;

    let is_detailed = serde_json::from_str::<serde_json::Value>(&objective)
        .map(|v| v.get("v").and_then(|v| v.as_str()) == Some("1"))
        .unwrap_or(false);
    let timestamp = now();
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE year_plan_lessons SET lesson_objective = ?1, detailed_plan_attached = ?2, updated_at = ?3 WHERE id = ?4",
            params![objective, is_detailed, timestamp, lesson.id],
        )
        .map_err(|e| e.to_string())?;
        update_job_progress(conn, &job_id, 1, 1, Some("Detailed lesson plan generated"))?;
        complete_job(conn, &job_id, Some("Detailed lesson plan generated"))?;
        let plan_id = class.year_plan_id.ok_or_else(|| "No year plan linked".to_string())?;
        load_lessons(conn, &plan_id)
            .map(|mut lessons| lessons.remove(lessons.iter().position(|l| l.id == lesson_id).unwrap_or(0)))
    })
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttachDetailedPlansResult {
    pub attached_count: i32,
    pub already_attached_count: i32,
    pub unmatched_count: i32,
    pub total_detailed_count: i32,
}

#[tauri::command]
pub async fn attach_detailed_plans_to_calendar_lessons(
    state: DbState<'_>,
    class_id: String,
) -> Result<AttachDetailedPlansResult, String> {
    state.with_conn(|conn| {
        let class = get_class(conn, &class_id).map_err(|e| e.to_string())?;
        let plan_id = class
            .year_plan_id
            .ok_or_else(|| "No lesson schedule found. Please generate a lesson schedule before attaching detailed plans.".to_string())?;
        let lessons = load_lessons(conn, &plan_id)?;
        if lessons.is_empty() {
            return Err("No lesson schedule found. Please generate a lesson schedule before attaching detailed plans.".to_string());
        }

        let mut attached_count = 0i32;
        let mut already_attached_count = 0i32;
        let mut unmatched_count = 0i32;
        let mut total_detailed_count = 0i32;

        for lesson in lessons.iter().filter(|lesson| !lesson.is_buffer) {
            let is_detailed = is_detailed_plan_json(lesson.lesson_objective.as_deref());
            if !is_detailed {
                continue;
            }
            total_detailed_count += 1;
            if lesson.detailed_plan_attached {
                already_attached_count += 1;
                continue;
            }
            let affected = conn
                .execute(
                    "UPDATE year_plan_lessons SET detailed_plan_attached = 1, updated_at = ?1 WHERE id = ?2",
                    params![now(), lesson.id],
                )
                .map_err(|e| e.to_string())?;
            if affected == 1 {
                attached_count += 1;
            } else {
                unmatched_count += 1;
            }
        }

        Ok(AttachDetailedPlansResult {
            attached_count,
            already_attached_count,
            unmatched_count,
            total_detailed_count,
        })
    })
}

fn build_lesson_preloads(
    conn: &rusqlite::Connection,
    lessons: &[YearPlanLesson],
    _plan_id: &str,
    _regenerate: bool,
) -> Result<Vec<LessonPlanPreload>, String> {
    let mut preloads = Vec::new();
    for lesson in lessons {
        let has_existing_plan = is_detailed_plan_json(lesson.lesson_objective.as_deref());
        if lesson.is_buffer {
            preloads.push(LessonPlanPreload {
                lesson_id: lesson.id.clone(),
                unit_title: String::new(),
                unit_description: None,
                unit_assessment: None,
                lesson_title: lesson.lesson_title.clone(),
                teaching_date: lesson.teaching_date.clone(),
                part_index: 0,
                total_parts: 0,
                is_buffer: true,
                has_existing_plan,
            });
            continue;
        }
        let unit_id = lesson.curriculum_unit_id.clone().ok_or_else(|| {
            format!(
                "lesson '{}' is missing syllabus unit mapping",
                lesson.lesson_title
            )
        })?;
        let (unit_title, unit_description, unit_assessment): (
            String,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT title, description, assessment_hint FROM curriculum_units WHERE id = ?1",
                params![unit_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| {
                "could not load syllabus unit details for one or more scheduled lessons".to_string()
            })?;

        let unit_indices: Vec<usize> = lessons
            .iter()
            .enumerate()
            .filter_map(|(idx, l)| {
                if l.curriculum_unit_id.as_deref() == lesson.curriculum_unit_id.as_deref() {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        let total_parts = unit_indices.len().max(1);
        let lesson_idx = lessons.iter().position(|l| l.id == lesson.id).unwrap_or(0);
        let part_index = unit_indices
            .iter()
            .position(|idx| *idx == lesson_idx)
            .map(|idx| idx + 1)
            .unwrap_or(1);

        preloads.push(LessonPlanPreload {
            lesson_id: lesson.id.clone(),
            unit_title,
            unit_description,
            unit_assessment,
            lesson_title: lesson.lesson_title.clone(),
            teaching_date: lesson.teaching_date.clone(),
            part_index,
            total_parts,
            is_buffer: false,
            has_existing_plan,
        });
    }
    Ok(preloads)
}

fn generate_lesson_objective_with_llm(
    config: &crate::services::llm_service::LocalModelConfig,
    class_name: &str,
    subject_name: Option<&str>,
    unit_title: &str,
    description: &Option<String>,
    assessment_hint: &Option<String>,
    lesson_title: &str,
    lesson_date: &str,
    part: usize,
    total_parts: usize,
    period_minutes: i32,
    context_notes: Option<&str>,
    template_content: Option<&str>,
) -> Result<String, String> {
    let desc = description.as_deref().unwrap_or(unit_title);
    let assessment = assessment_hint
        .as_deref()
        .unwrap_or("Teacher observation and formative check");
    let subject = subject_name.unwrap_or("General subject");
    let context = context_notes.unwrap_or("");
    let template_instruction = template_content
        .filter(|text| !text.trim().is_empty())
        .map(|template| format!(
            "\nFollow this uploaded lesson-plan template structure and language where possible while still returning valid JSON keys:\n{}",
            template.chars().take(6000).collect::<String>(),
        ))
        .unwrap_or_default();
    let prompt = format!(
        "You are generating one classroom-ready BACKWARD DESIGN lesson plan in JSON only.\n\
Return only a JSON object with these exact keys and types.\n\
Top-level required keys:\n\
v:string (must be \"1\"), objective:string, transferGoal:string, enduringUnderstanding:string,\n\
essentialQuestions:string[], knowledge:string[], skills:string[], successCriteria:string[],\n\
assessment:string, performanceTask:string, materials:string[], vocabulary:string[],\n\
lessonFlow:{{phase:string,time:string,description:string}}[], differentiation:string,\n\
homework:string, crossCurricular:string,\n\
backwardDesign:object.\n\
\n\
The backwardDesign object must include these sections (all required):\n\
lessonInformation:object,\n\
stage1DesiredResults:object,\n\
stage2AssessmentEvidence:object,\n\
stage3LearningPlan:object,\n\
differentiation:object,\n\
resourcesAndMaterials:object,\n\
timingBreakdown:object,\n\
checkingForUnderstandingQuestions:object,\n\
crossCurricularRealWorldConnections:string[],\n\
classroomManagementConsiderations:object,\n\
homeworkFollowUp:object,\n\
teacherReflection:object.\n\
\n\
Use the provided backward-design template language and order. Keep it practical for one lesson.\n\
No markdown. No prose outside JSON.\n\n\
Class: {class_name}\n\
Subject: {subject}\n\
Unit: {unit_title}\n\
Scheduled lesson: {lesson_title}\n\
Date: {lesson_date}\n\
Lesson in sequence: {part}/{total_parts}\n\
Lesson length: {period_minutes} minutes\n\
Planned focus notes: {desc}\n\
Assessment intent: {assessment}\n\
Teacher context: {context}\n\n\
Make the plan realistic for one lesson, progressive for this sequence position, and specific to the provided focus.\n\
The stage3LearningPlan object must have these exact keys: hookWarmUp, activatePriorKnowledge, explicitTeachingModeling, guidedPractice, collaborativePractice, independentPractice, closureReflection, exitTicket. Each key must be a non-empty string describing that phase of the lesson.{}",
        template_instruction,
    );
    let mut errors: Vec<String> = Vec::new();
    let attempt_delays_ms = [0u64, 2_000, 5_000];
    for (attempt, delay_ms) in attempt_delays_ms.iter().enumerate() {
        if *delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(*delay_ms));
        }
        let raw_prompt = if attempt == 0 {
            prompt.clone()
        } else {
            format!(
                "{prompt}\n\nYour previous response was not valid strict JSON. \
Return only one valid JSON object with quoted keys and no markdown/code fences."
            )
        };
        let raw = match generate_local_report(config, &raw_prompt) {
            Ok(value) => value,
            Err(err) => {
                let cleaned = sanitize_provider_error(&err);
                errors.push(cleaned.clone());
                if is_retryable_provider_error(&cleaned) && attempt + 1 < attempt_delays_ms.len() {
                    continue;
                }
                return Err(cleaned);
            }
        };
        match parse_and_validate_detailed_plan_value(&raw) {
            Ok(value) => return Ok(value.to_string()),
            Err(err) => errors.push(err),
        }
    }
    let first_error = errors
        .first()
        .cloned()
        .unwrap_or_else(|| "model returned invalid JSON".to_string());
    if should_use_fallback_plan(&errors) {
        let fallback = build_fallback_detailed_plan(
            class_name,
            subject_name,
            unit_title,
            desc,
            lesson_title,
            lesson_date,
            part,
            total_parts,
            period_minutes,
            assessment,
        );
        return Ok(fallback.to_string());
    }
    Err(first_error)
}

fn should_use_fallback_plan(errors: &[String]) -> bool {
    if !allow_fallback_detailed_plans() {
        return false;
    }
    if errors.is_empty() {
        return false;
    }
    errors.iter().all(|err| {
        let lower = err.to_ascii_lowercase();
        lower.contains("429")
            || lower.contains("too many requests")
            || lower.contains("rate limit")
            || lower.contains("timeout")
            || lower.contains("timed out")
            || lower.contains("connection")
            || lower.contains("network")
            || lower.contains("failed to start")
            || lower.contains("no such file")
            || lower.contains("not found")
    })
}

fn allow_fallback_detailed_plans() -> bool {
    let Ok(value) = std::env::var("EDUTRACK_ALLOW_FALLBACK_DETAILED_PLANS") else {
        return false;
    };
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn is_retryable_provider_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("429")
        || lower.contains("too many requests")
        || lower.contains("rate limit")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection")
        || lower.contains("network")
        || lower.contains("temporary")
}

fn build_fallback_detailed_plan(
    class_name: &str,
    subject_name: Option<&str>,
    unit_title: &str,
    desc: &str,
    lesson_title: &str,
    lesson_date: &str,
    part: usize,
    total_parts: usize,
    period_minutes: i32,
    assessment: &str,
) -> serde_json::Value {
    let subject = subject_name.unwrap_or("General subject");
    let transition_note = if part == 1 {
        "Launch this unit by activating prior knowledge and setting clear success criteria."
    } else if part >= total_parts {
        "Consolidate the unit by synthesizing learning and preparing for summative checks."
    } else {
        "Advance the unit progression through guided-to-independent practice."
    };
    let hook_minutes = (period_minutes as f32 * 0.12).round() as i32;
    let model_minutes = (period_minutes as f32 * 0.25).round() as i32;
    let guided_minutes = (period_minutes as f32 * 0.25).round() as i32;
    let independent_minutes = (period_minutes as f32 * 0.25).round() as i32;
    let closure_minutes =
        (period_minutes - hook_minutes - model_minutes - guided_minutes - independent_minutes)
            .max(5);

    serde_json::json!({
        "v": "1",
        "generationMode": "fallback",
        "objective": format!("Students will demonstrate progress on '{}' by applying the lesson focus in class tasks.", unit_title),
        "transferGoal": "Apply strategies independently in new contexts.",
        "enduringUnderstanding": "Strong understanding is built through evidence, explanation, and reflection.",
        "essentialQuestions": [
            format!("What makes '{}' effective or meaningful in context?", unit_title),
            "How can I justify my thinking with clear evidence?"
        ],
        "knowledge": [
            desc,
            format!("Key concepts from {} ({}/{})", unit_title, part, total_parts)
        ],
        "skills": [
            "Interpret and explain evidence",
            "Collaborate and communicate reasoning clearly",
            "Revise work using feedback"
        ],
        "successCriteria": [
            "I can complete the lesson task with accurate content.",
            "I can explain my thinking with evidence from the lesson material.",
            "I can reflect on one improvement for next lesson."
        ],
        "assessment": assessment,
        "performanceTask": format!("Complete a focused task tied to '{}', then justify decisions with evidence.", lesson_title),
        "materials": [
            "Teacher slides or board notes",
            "Student notebook or worksheet",
            "Core reading/resource excerpt",
            "Exit ticket prompt"
        ],
        "vocabulary": [
            unit_title,
            "evidence",
            "analysis",
            "reflection"
        ],
        "lessonFlow": [
            {"phase":"Hook/Warm-Up","time":format!("{hook_minutes} min"),"description":"Brief prompt to activate interest and prior learning."},
            {"phase":"Explicit Teaching/Modeling","time":format!("{model_minutes} min"),"description":"Teacher models strategy and success criteria."},
            {"phase":"Guided + Collaborative Practice","time":format!("{guided_minutes} min"),"description":"Students practice with teacher scaffolding and peer discussion."},
            {"phase":"Independent Practice","time":format!("{independent_minutes} min"),"description":"Students apply learning individually to demonstrate mastery."},
            {"phase":"Closure/Exit Ticket","time":format!("{closure_minutes} min"),"description":"Students summarize learning and submit evidence of understanding."}
        ],
        "differentiation": "Use tiered prompts, sentence frames, and optional challenge extensions based on readiness.",
        "homework": "Refine todayâ€™s task and prepare one question for the next class.",
        "crossCurricular": "Connect ideas to real-world communication, media, and analytical thinking.",
        "backwardDesign": {
            "lessonInformation": {
                "class": class_name,
                "subject": subject,
                "unit": unit_title,
                "lessonTitle": lesson_title,
                "date": lesson_date,
                "sequencePosition": format!("{part}/{total_parts}"),
                "durationMinutes": period_minutes
            },
            "stage1DesiredResults": {
                "transferGoal": "Students independently apply the dayâ€™s strategy in future learning tasks.",
                "enduringUnderstanding": "Learning deepens when students justify choices with evidence.",
                "essentialQuestions": [
                    "What strategy did I use and why?",
                    "How does evidence strengthen my answer?"
                ],
                "knowledgeAndSkills": [
                    desc,
                    "Evidence-based explanation"
                ]
            },
            "stage2AssessmentEvidence": {
                "assessmentSummary": assessment,
                "performanceTask": "In-class task + explanation",
                "formativeChecks": ["Cold call checks", "Guided practice observations", "Exit ticket"]
            },
            "stage3LearningPlan": {
                "hookWarmUp": "Prompt students with a short, relevant question tied to the unit theme.",
                "activatePriorKnowledge": "Review one key idea from the previous lesson and connect to todayâ€™s objective.",
                "explicitTeachingModeling": "Demonstrate the target approach with think-aloud and annotated example.",
                "guidedPractice": "Students practice one item with structured teacher support.",
                "collaborativePractice": "Pairs/small groups compare responses and refine evidence use.",
                "independentPractice": "Students complete an individual task that mirrors the success criteria.",
                "closureReflection": transition_note,
                "exitTicket": "Submit one evidence-backed response plus one self-assessment note."
            },
            "differentiation": {
                "support": "Chunk instructions, provide exemplars, and use guided notes.",
                "extension": "Add a comparison/justification challenge for early finishers."
            },
            "resourcesAndMaterials": {
                "core": ["Lesson text/resource", "Slides/board", "Practice task", "Exit ticket"]
            },
            "timingBreakdown": {
                "hookWarmUp": format!("{hook_minutes} min"),
                "explicitTeachingModeling": format!("{model_minutes} min"),
                "guidedCollaborativePractice": format!("{guided_minutes} min"),
                "independentPractice": format!("{independent_minutes} min"),
                "closureExitTicket": format!("{closure_minutes} min")
            },
            "checkingForUnderstandingQuestions": {
                "q1": "What evidence supports your answer?",
                "q2": "Which strategy did you use and why?",
                "q3": "What would you improve in your response?"
            },
            "crossCurricularRealWorldConnections": [
                "Analytical writing and communication",
                "Media interpretation and critical thinking"
            ],
            "classroomManagementConsiderations": {
                "grouping": "Use mixed-readiness pairs for discussion.",
                "transitions": "Use clear countdowns between phases."
            },
            "homeworkFollowUp": {
                "task": "Revise and extend todayâ€™s response.",
                "nextLessonBridge": "Bring one question or confusion point."
            },
            "teacherReflection": {
                "prompt": "Which supports moved students toward independent evidence-based responses?"
            }
        }
    })
}

fn parse_and_validate_detailed_plan_value(raw: &str) -> Result<serde_json::Value, String> {
    let candidates = collect_json_candidates(raw);
    if candidates.is_empty() {
        return Err("model output did not contain JSON".to_string());
    }
    let mut parse_errors = Vec::new();
    for candidate in candidates {
        match parse_detailed_json_candidate(&candidate) {
            Ok(mut value) => {
                if value.get("v").and_then(|v| v.as_str()) != Some("1") {
                    value["v"] = serde_json::Value::String("1".to_string());
                }
                normalize_detailed_plan_value(&mut value);
                validate_detailed_plan_json(&value)?;
                return Ok(value);
            }
            Err(err) => parse_errors.push(err),
        }
    }
    if parse_errors.is_empty() {
        Err("model returned invalid JSON".to_string())
    } else {
        Err(parse_errors.remove(0))
    }
}

fn parse_detailed_json_candidate(snippet: &str) -> Result<serde_json::Value, String> {
    let mut last_error = String::new();
    for text in candidate_variants(snippet) {
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => return Ok(value),
            Err(err) => last_error = format!("invalid JSON: {err}"),
        }
    }
    if last_error.is_empty() {
        Err("invalid JSON".to_string())
    } else {
        Err(last_error)
    }
}

fn candidate_variants(text: &str) -> Vec<String> {
    let mut variants = Vec::new();
    let bom_stripped = strip_utf8_bom(text);
    push_unique(&mut variants, bom_stripped.trim().to_string());
    let stripped = strip_disallowed_control_chars(&bom_stripped);
    push_unique(&mut variants, stripped.clone());
    let no_trailing_commas = strip_trailing_commas(&stripped);
    push_unique(&mut variants, no_trailing_commas.clone());
    let normalized_quotes = normalize_smart_quotes(&no_trailing_commas);
    let normalized_unicode_quotes = normalize_unicode_quotes(&normalized_quotes);
    push_unique(&mut variants, normalized_unicode_quotes.clone());
    let quoted_keys = quote_unquoted_json_keys(&normalized_unicode_quotes);
    push_unique(&mut variants, quoted_keys);
    variants
}

fn collect_json_candidates(raw: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let trimmed = strip_utf8_bom(raw);
    let trimmed = trimmed.trim();
    if !trimmed.is_empty() {
        push_unique(&mut candidates, trimmed.to_string());
    }

    for block in extract_fenced_json_candidates(raw) {
        push_unique(&mut candidates, block);
    }
    for fragment in extract_response_field_candidates(raw) {
        push_unique(&mut candidates, fragment);
    }
    for object in extract_balanced_json_fragments(raw, '{', '}') {
        push_unique(&mut candidates, object);
    }

    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) {
        if end >= start {
            push_unique(&mut candidates, raw[start..=end].to_string());
        }
    }
    candidates
}

fn push_unique(items: &mut Vec<String>, candidate: String) {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    if !items.iter().any(|existing| existing == trimmed) {
        items.push(trimmed.to_string());
    }
}

fn extract_fenced_json_candidates(raw: &str) -> Vec<String> {
    let fence_re = Regex::new(r"(?s)```(?:json)?\s*(.*?)\s*```").expect("valid fenced-json regex");
    let mut snippets = Vec::new();
    for cap in fence_re.captures_iter(raw) {
        let Some(block) = cap.get(1).map(|m| m.as_str()) else {
            continue;
        };
        push_unique(&mut snippets, block.to_string());
        if let (Some(start), Some(end)) = (block.find('{'), block.rfind('}')) {
            if end >= start {
                push_unique(&mut snippets, block[start..=end].to_string());
            }
        }
    }
    snippets
}

fn strip_disallowed_control_chars(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .collect()
}

fn strip_utf8_bom(text: &str) -> String {
    text.trim_start_matches('\u{feff}').to_string()
}

fn strip_trailing_commas(text: &str) -> String {
    let re = Regex::new(r",\s*([}\]])").expect("valid trailing comma regex");
    re.replace_all(text, "$1").to_string()
}

fn normalize_unicode_quotes(text: &str) -> String {
    text.replace(['\u{201C}', '\u{201D}'], "\"")
        .replace(['\u{2019}', '\u{2018}'], "'")
}

fn extract_response_field_candidates(raw: &str) -> Vec<String> {
    let mut snippets = Vec::new();
    let mut combined_response = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        if let Some(response) = value.get("response").and_then(|v| v.as_str()) {
            combined_response.push_str(response);
        }
        if let Some(content) = value
            .get("message")
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
        {
            push_unique(&mut snippets, content.to_string());
        }
    }
    if !combined_response.trim().is_empty() {
        push_unique(&mut snippets, combined_response.clone());
        for object in extract_balanced_json_fragments(&combined_response, '{', '}') {
            push_unique(&mut snippets, object);
        }
    }
    snippets
}

fn extract_balanced_json_fragments(raw: &str, open: char, close: char) -> Vec<String> {
    let mut snippets = Vec::new();
    for (idx, ch) in raw.char_indices() {
        if ch != open {
            continue;
        }
        let Some(end) = find_matching_bracket(raw, idx, open, close) else {
            continue;
        };
        push_unique(&mut snippets, raw[idx..=end].to_string());
    }
    snippets
}

fn find_matching_bracket(text: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == open {
            depth += 1;
        } else if ch == close {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                return Some(start + offset);
            }
        }
    }
    None
}

fn normalize_smart_quotes(text: &str) -> String {
    text.replace('\u{201C}', "\"")
        .replace('\u{201D}', "\"")
        .replace('\u{2019}', "'")
        .replace('\u{2018}', "'")
}

fn quote_unquoted_json_keys(text: &str) -> String {
    let key_re = Regex::new(r#"([{\[,]\s*)([A-Za-z_][A-Za-z0-9_]*)(\s*:)"#)
        .expect("valid unquoted-key regex");
    key_re.replace_all(text, "$1\"$2\"$3").to_string()
}

fn sanitize_provider_error(message: &str) -> String {
    let csi_re = Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").expect("valid ansi-csi regex");
    let osc_re = Regex::new(r"\x1b\][^\x1b\x07]*(?:\x07|\x1b\\)").expect("valid ansi-osc regex");
    let vt_fragment_re =
        Regex::new(r"\[\?[0-9;]*[A-Za-z]|\[[0-9;]*[A-Za-z]|\[K").expect("valid vt-fragment regex");
    let mut cleaned = csi_re.replace_all(message, "").to_string();
    cleaned = osc_re.replace_all(&cleaned, "").to_string();
    cleaned = vt_fragment_re.replace_all(&cleaned, " ").to_string();
    let cleaned: String = cleaned
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .collect();
    let mut compact = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = compact.to_ascii_lowercase();
    if let Some(idx) = lower.find("error: 429") {
        compact = compact[idx..].to_string();
    } else if let Some(idx) = lower.find("429 too many requests") {
        compact = compact[idx..].to_string();
    }
    if compact.is_empty() {
        "model provider returned an empty error".to_string()
    } else {
        compact
    }
}

fn normalize_detailed_plan_value(value: &mut serde_json::Value) {
    if !value.is_object() {
        return;
    }
    let snapshot = value.clone();
    let lesson_title = first_non_empty_string(
        &snapshot,
        &[
            &["backwardDesign", "lessonInformation", "lessonTitle"],
            &["backwardDesign", "lessonInformation", "unit"],
        ],
    )
    .unwrap_or_else(|| "this lesson".to_string());
    let (unit_label, topic_label, keyword_terms) = lesson_context_from_title(&lesson_title);
    let primary_term = keyword_terms
        .first()
        .cloned()
        .unwrap_or_else(|| topic_label.clone());

    set_string_if_missing(
        value,
        "objective",
        first_non_empty_string(
            &snapshot,
            &[
                &["objective"],
                &["backwardDesign", "stage1DesiredResults", "understanding"],
                &["backwardDesign", "stage1DesiredResults", "enduringUnderstanding"],
                &["backwardDesign", "stage2AssessmentEvidence", "performanceTask"],
            ],
        ),
        &format!(
            "Students will apply {} to build understanding of {} and explain reasoning with evidence.",
            topic_label, unit_label
        ),
    );
    set_string_if_missing(
        value,
        "transferGoal",
        first_non_empty_string(
            &snapshot,
            &[
                &["transferGoal"],
                &["backwardDesign", "stage1DesiredResults", "transferGoal"],
            ],
        ),
        &format!(
            "Independently apply {} in a new task and justify choices using evidence.",
            topic_label
        ),
    );
    set_string_if_missing(
        value,
        "enduringUnderstanding",
        first_non_empty_string(
            &snapshot,
            &[
                &["enduringUnderstanding"],
                &["backwardDesign", "stage1DesiredResults", "enduringUnderstanding"],
                &["backwardDesign", "stage1DesiredResults", "understanding"],
            ],
        ),
        &format!(
            "Understanding in {} deepens when students support claims about {} with precise evidence and reflection.",
            unit_label, topic_label
        ),
    );
    set_string_if_missing(
        value,
        "assessment",
        first_non_empty_string(
            &snapshot,
            &[
                &["assessment"],
                &[
                    "backwardDesign",
                    "stage2AssessmentEvidence",
                    "assessmentSummary",
                ],
                &["backwardDesign", "stage2AssessmentEvidence", "formative"],
            ],
        ),
        &format!(
            "Teacher observation during practice plus an exit ticket focused on {}.",
            topic_label
        ),
    );
    set_string_if_missing(
        value,
        "performanceTask",
        first_non_empty_string(
            &snapshot,
            &[
                &["performanceTask"],
                &[
                    "backwardDesign",
                    "stage2AssessmentEvidence",
                    "performanceTask",
                ],
            ],
        ),
        &format!(
            "Complete a task on {} and justify decisions using evidence from class work.",
            topic_label
        ),
    );
    set_string_if_missing(
        value,
        "differentiation",
        first_non_empty_string(
            &snapshot,
            &[
                &["differentiation"],
                &["backwardDesign", "differentiation", "support"],
            ],
        ),
        &format!(
            "Use tiered supports for {} and extension prompts for students ready to go deeper.",
            topic_label
        ),
    );
    set_string_if_missing(
        value,
        "homework",
        first_non_empty_string(
            &snapshot,
            &[
                &["homework"],
                &["backwardDesign", "homeworkFollowUp", "task"],
            ],
        ),
        &format!(
            "Review today's work on {} and write one evidence-based follow-up question.",
            topic_label
        ),
    );
    let cross_joined = first_non_empty_string_array(
        &snapshot,
        &[&["backwardDesign", "crossCurricularRealWorldConnections"]],
    )
    .map(|items| items.join(", "));
    set_string_if_missing(
        value,
        "crossCurricular",
        first_non_empty_string(&snapshot, &[&["crossCurricular"]]).or(cross_joined),
        &format!(
            "Connect {} to analytical communication across subjects.",
            topic_label
        ),
    );

    ensure_string_array(
        value,
        "essentialQuestions",
        &snapshot,
        &[&["essentialQuestions"]],
        &format!(
            "How can I apply {} accurately and explain why it works?",
            topic_label
        ),
    );
    ensure_string_array(
        value,
        "knowledge",
        &snapshot,
        &[
            &["knowledge"],
            &[
                "backwardDesign",
                "stage1DesiredResults",
                "knowledgeAndSkills",
            ],
        ],
        &format!(
            "Core concepts and examples for {} in {}.",
            topic_label, unit_label
        ),
    );
    ensure_string_array(
        value,
        "skills",
        &snapshot,
        &[
            &["skills"],
            &[
                "backwardDesign",
                "stage1DesiredResults",
                "knowledgeAndSkills",
            ],
        ],
        &format!(
            "Apply {} and explain reasoning with lesson evidence.",
            topic_label
        ),
    );
    ensure_string_array(
        value,
        "successCriteria",
        &snapshot,
        &[&["successCriteria"]],
        &format!(
            "I can complete a {} task and justify my answer with accurate evidence.",
            topic_label
        ),
    );
    ensure_string_array(
        value,
        "materials",
        &snapshot,
        &[
            &["materials"],
            &["backwardDesign", "resourcesAndMaterials", "items"],
            &["backwardDesign", "resourcesAndMaterials", "core"],
        ],
        &format!(
            "Lesson materials for {} and student evidence notes.",
            topic_label
        ),
    );
    ensure_string_array(
        value,
        "vocabulary",
        &snapshot,
        &[&["vocabulary"]],
        &primary_term,
    );

    ensure_lesson_flow(value, &snapshot);
    ensure_backward_design_shape(value);
    normalize_stage3_learning_plan_keys(value);
    replace_generic_plan_placeholders(value, &unit_label, &topic_label, &keyword_terms);
}

fn lesson_context_from_title(lesson_title: &str) -> (String, String, Vec<String>) {
    let mut parts = lesson_title.splitn(2, ':');
    let unit = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("the unit")
        .to_string();
    let topic = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(unit.as_str())
        .to_string();

    let mut keywords: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let stop_words = [
        "unit", "lesson", "the", "and", "for", "with", "from", "into", "about", "class",
        "students", "student", "core", "part", "day", "week", "month", "year",
    ];
    let cleaned = format!("{unit} {topic}");
    for token in cleaned
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let lower = token.to_ascii_lowercase();
        if lower.len() < 3 || stop_words.contains(&lower.as_str()) {
            continue;
        }
        if seen.insert(lower) {
            keywords.push(token.to_string());
        }
        if keywords.len() >= 6 {
            break;
        }
    }
    if keywords.is_empty() {
        keywords.push(topic.clone());
    }
    (unit, topic, keywords)
}

fn replace_generic_plan_placeholders(
    value: &mut serde_json::Value,
    unit: &str,
    topic: &str,
    keyword_terms: &[String],
) {
    let generic_string_replacements: [(&str, &str); 6] = [
        (
            "objective",
            "Students will make measurable progress in this lesson",
        ),
        ("transferGoal", "Apply the lesson strategy independently."),
        (
            "enduringUnderstanding",
            "Strong understanding is built through evidence and reflection.",
        ),
        ("assessment", "Teacher observation and exit ticket."),
        (
            "performanceTask",
            "Complete the core lesson task and justify thinking with evidence.",
        ),
        (
            "differentiation",
            "Use supports and extensions based on readiness.",
        ),
    ];

    for (key, generic) in generic_string_replacements {
        let existing = value
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if existing == generic.to_ascii_lowercase() {
            let replacement = match key {
                "objective" => format!("Students will apply {topic} in {unit} and justify reasoning with evidence."),
                "transferGoal" => format!("Apply {topic} independently in a new context."),
                "enduringUnderstanding" => format!("Deep understanding of {unit} comes from evidence-based explanations about {topic}."),
                "assessment" => format!("Teacher observation plus an exit ticket targeting {topic}."),
                "performanceTask" => format!("Complete a {topic} task and justify each decision with evidence."),
                _ => format!("Use tiered supports and extensions while learning {topic}."),
            };
            value[key] = serde_json::Value::String(replacement);
        }
    }

    let array_generic_replacements: [(&str, &str, String); 5] = [
        (
            "essentialQuestions",
            "What strategy helps me succeed in this lesson?",
            format!("How can I apply {topic} accurately and explain why it works?"),
        ),
        (
            "knowledge",
            "Key lesson concepts and examples.",
            format!("Key concepts and examples for {topic} in {unit}."),
        ),
        (
            "skills",
            "Apply the target strategy and explain reasoning.",
            format!("Apply {topic} and explain reasoning with evidence."),
        ),
        (
            "successCriteria",
            "I can complete the task with accurate evidence.",
            format!("I can complete a {topic} task and justify my answer with accurate evidence."),
        ),
        (
            "materials",
            "Lesson materials and student notes.",
            format!("Materials and note-taking tools needed for {topic}."),
        ),
    ];
    for (key, generic, replacement) in array_generic_replacements {
        let Some(items) = value.get_mut(key).and_then(|v| v.as_array_mut()) else {
            continue;
        };
        for item in items.iter_mut() {
            let matches_generic = item
                .as_str()
                .map(str::trim)
                .map(|text| text.eq_ignore_ascii_case(generic))
                .unwrap_or(false);
            if matches_generic {
                *item = serde_json::Value::String(replacement.clone());
            }
        }
    }

    if let Some(vocab_items) = value.get_mut("vocabulary").and_then(|v| v.as_array_mut()) {
        for item in vocab_items.iter_mut() {
            let matches_placeholder = item
                .as_str()
                .map(str::trim)
                .map(|text| text.eq_ignore_ascii_case("key term"))
                .unwrap_or(false);
            if matches_placeholder {
                let replacement = keyword_terms
                    .first()
                    .cloned()
                    .unwrap_or_else(|| topic.to_string());
                *item = serde_json::Value::String(replacement);
            }
        }
    }
}
fn first_non_empty_string(value: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        let mut cursor = value;
        let mut found = true;
        for key in *path {
            if let Some(next) = cursor.get(*key) {
                cursor = next;
            } else {
                found = false;
                break;
            }
        }
        if !found {
            continue;
        }
        if let Some(s) = cursor.as_str().map(str::trim).filter(|s| !s.is_empty()) {
            return Some(s.to_string());
        }
    }
    None
}

fn first_non_empty_string_array(
    value: &serde_json::Value,
    paths: &[&[&str]],
) -> Option<Vec<String>> {
    for path in paths {
        let mut cursor = value;
        let mut found = true;
        for key in *path {
            if let Some(next) = cursor.get(*key) {
                cursor = next;
            } else {
                found = false;
                break;
            }
        }
        if !found {
            continue;
        }
        if let Some(items) = cursor.as_array() {
            let values: Vec<String> = items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim))
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect();
            if !values.is_empty() {
                return Some(values);
            }
        }
    }
    None
}

fn set_string_if_missing(
    value: &mut serde_json::Value,
    key: &str,
    candidate: Option<String>,
    default_value: &str,
) {
    let needs_value = value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_none();
    if !needs_value {
        return;
    }
    let text = candidate
        .unwrap_or_else(|| default_value.to_string())
        .trim()
        .to_string();
    let final_text = if text.is_empty() {
        default_value.to_string()
    } else {
        text
    };
    value[key] = serde_json::Value::String(final_text);
}

fn ensure_string_array(
    value: &mut serde_json::Value,
    key: &str,
    snapshot: &serde_json::Value,
    source_paths: &[&[&str]],
    default_item: &str,
) {
    let existing = value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim))
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty());
    if let Some(items) = existing {
        value[key] =
            serde_json::Value::Array(items.into_iter().map(serde_json::Value::String).collect());
        return;
    }
    let source = first_non_empty_string_array(snapshot, source_paths)
        .unwrap_or_else(|| vec![default_item.to_string()]);
    value[key] =
        serde_json::Value::Array(source.into_iter().map(serde_json::Value::String).collect());
}

fn ensure_lesson_flow(value: &mut serde_json::Value, snapshot: &serde_json::Value) {
    let has_flow = value
        .get("lessonFlow")
        .and_then(|v| v.as_array())
        .map(|items| items.len() >= 3)
        .unwrap_or(false);
    if has_flow {
        return;
    }
    let mut flow = Vec::new();
    if let Some(stage3) = snapshot
        .get("backwardDesign")
        .and_then(|v| v.get("stage3LearningPlan"))
        .and_then(|v| v.as_object())
    {
        let map = [
            ("hookWarmUp", "Hook/Warm-Up"),
            ("explicitTeachingModeling", "Explicit Teaching/Modeling"),
            ("guidedPractice", "Guided Practice"),
            ("independentPractice", "Independent Practice"),
            ("closureReflection", "Closure/Reflection"),
        ];
        for (key, phase) in map {
            if let Some(desc) = stage3
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                flow.push(
                    serde_json::json!({"phase": phase, "time": "8 min", "description": desc}),
                );
            }
        }
    }
    if flow.len() < 3 {
        flow = vec![
            serde_json::json!({"phase":"Hook/Warm-Up","time":"8 min","description":"Activate prior knowledge and set the lesson goal."}),
            serde_json::json!({"phase":"Guided + Independent Practice","time":"30 min","description":"Model, practice, and apply the target skill with evidence."}),
            serde_json::json!({"phase":"Closure/Exit Ticket","time":"7 min","description":"Check understanding and capture next steps."}),
        ];
    }
    value["lessonFlow"] = serde_json::Value::Array(flow);
}

fn ensure_backward_design_shape(value: &mut serde_json::Value) {
    if !value
        .get("backwardDesign")
        .map(|v| v.is_object())
        .unwrap_or(false)
    {
        value["backwardDesign"] = serde_json::json!({});
    }
    let backward = value
        .get_mut("backwardDesign")
        .and_then(|v| v.as_object_mut())
        .expect("backwardDesign must be object");
    for key in [
        "lessonInformation",
        "stage1DesiredResults",
        "stage2AssessmentEvidence",
        "stage3LearningPlan",
        "differentiation",
        "resourcesAndMaterials",
        "timingBreakdown",
        "checkingForUnderstandingQuestions",
        "classroomManagementConsiderations",
        "homeworkFollowUp",
        "teacherReflection",
    ] {
        if !backward.get(key).map(|v| v.is_object()).unwrap_or(false) {
            backward.insert(key.to_string(), serde_json::json!({}));
        }
    }
    if !backward
        .get("crossCurricularRealWorldConnections")
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false)
    {
        backward.insert(
            "crossCurricularRealWorldConnections".to_string(),
            serde_json::json!(["Cross-subject analytical communication"]),
        );
    }
}

fn normalize_stage3_learning_plan_keys(value: &mut serde_json::Value) {
    let canonical: &[(&str, &[&str])] = &[
        (
            "hookWarmUp",
            &[
                "hookWarmUp",
                "Hook/Warm-Up",
                "hook_warm_up",
                "hook warm up",
                "hookwarmup",
                "warmup",
                "warm-up",
            ],
        ),
        (
            "activatePriorKnowledge",
            &[
                "activatePriorKnowledge",
                "Activate Prior Knowledge",
                "activate_prior_knowledge",
                "activate prior knowledge",
                "prior knowledge",
                "prior_knowledge",
            ],
        ),
        (
            "explicitTeachingModeling",
            &[
                "explicitTeachingModeling",
                "Explicit Teaching/Modeling",
                "explicit_teaching_modeling",
                "explicit teaching/modeling",
                "explicit teaching and modeling",
                "direct instruction",
                "direct_instruction",
            ],
        ),
        (
            "guidedPractice",
            &[
                "guidedPractice",
                "Guided Practice",
                "guided_practice",
                "guided practice",
            ],
        ),
        (
            "collaborativePractice",
            &[
                "collaborativePractice",
                "Collaborative Practice",
                "collaborative_practice",
                "collaborative practice",
                "group work",
                "group_work",
            ],
        ),
        (
            "independentPractice",
            &[
                "independentPractice",
                "Independent Practice",
                "independent_practice",
                "independent practice",
            ],
        ),
        (
            "closureReflection",
            &[
                "closureReflection",
                "Closure/Reflection",
                "closure_reflection",
                "closure/reflection",
                "closure",
                "closing",
            ],
        ),
        (
            "exitTicket",
            &[
                "exitTicket",
                "Exit Ticket",
                "exit_ticket",
                "exit ticket",
                "exit",
            ],
        ),
    ];

    let Some(stage3) = value
        .get_mut("backwardDesign")
        .and_then(|v| v.get_mut("stage3LearningPlan"))
        .and_then(|v| v.as_object_mut())
    else {
        return;
    };

    let defaults: &[(&str, &str)] = &[
        (
            "hookWarmUp",
            "Warm-up activity to engage students and connect to prior knowledge.",
        ),
        (
            "activatePriorKnowledge",
            "Review and connect previous learning to today's objective.",
        ),
        (
            "explicitTeachingModeling",
            "Teacher models the target skill with clear explanation.",
        ),
        (
            "guidedPractice",
            "Structured practice with teacher support and feedback.",
        ),
        (
            "collaborativePractice",
            "Peer collaboration to deepen understanding through discussion.",
        ),
        (
            "independentPractice",
            "Independent application of the lesson objective.",
        ),
        (
            "closureReflection",
            "Summarize key takeaways and reflect on learning.",
        ),
        (
            "exitTicket",
            "Formative check to assess understanding of the lesson goal.",
        ),
    ];

    for &(canonical_key, aliases) in canonical {
        if stage3.contains_key(canonical_key) {
            continue;
        }
        let mut found = false;
        for &alias in aliases {
            if let Some((_, value)) = stage3.remove_entry(alias) {
                stage3.insert(canonical_key.to_string(), value);
                found = true;
                break;
            }
        }
        if !found {
            let default = defaults
                .iter()
                .find(|&&(k, _)| k == canonical_key)
                .map(|&(_, v)| v)
                .unwrap_or("");
            stage3.insert(
                canonical_key.to_string(),
                serde_json::Value::String(default.to_string()),
            );
        }
    }
}

fn validate_detailed_plan_json(value: &serde_json::Value) -> Result<(), String> {
    let required_strings = [
        "v",
        "objective",
        "transferGoal",
        "enduringUnderstanding",
        "assessment",
        "performanceTask",
        "differentiation",
        "homework",
        "crossCurricular",
    ];
    for key in required_strings {
        if value
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Err(format!("missing or empty '{key}'"));
        }
    }
    let required_arrays = [
        "essentialQuestions",
        "knowledge",
        "skills",
        "successCriteria",
        "materials",
        "vocabulary",
    ];
    for key in required_arrays {
        let Some(items) = value.get(key).and_then(|v| v.as_array()) else {
            return Err(format!("missing array '{key}'"));
        };
        if items.is_empty() {
            return Err(format!("array '{key}' must not be empty"));
        }
        if items.iter().any(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_none()
        }) {
            return Err(format!("array '{key}' must contain non-empty strings"));
        }
    }
    let Some(flow) = value.get("lessonFlow").and_then(|v| v.as_array()) else {
        return Err("missing array 'lessonFlow'".to_string());
    };
    if flow.len() < 3 {
        return Err("lessonFlow must have at least 3 stages".to_string());
    }
    for step in flow {
        let Some(obj) = step.as_object() else {
            return Err("lessonFlow items must be objects".to_string());
        };
        for key in ["phase", "time", "description"] {
            if obj
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_none()
            {
                return Err(format!("lessonFlow item missing '{key}'"));
            }
        }
    }

    let Some(backward) = value.get("backwardDesign").and_then(|v| v.as_object()) else {
        return Err("missing object 'backwardDesign'".to_string());
    };

    let required_backward_objects = [
        "lessonInformation",
        "stage1DesiredResults",
        "stage2AssessmentEvidence",
        "stage3LearningPlan",
        "differentiation",
        "resourcesAndMaterials",
        "timingBreakdown",
        "checkingForUnderstandingQuestions",
        "classroomManagementConsiderations",
        "homeworkFollowUp",
        "teacherReflection",
    ];
    for key in required_backward_objects {
        if backward.get(key).and_then(|v| v.as_object()).is_none() {
            return Err(format!("backwardDesign missing object '{key}'"));
        }
    }
    let Some(xcurr) = backward
        .get("crossCurricularRealWorldConnections")
        .and_then(|v| v.as_array())
    else {
        return Err(
            "backwardDesign missing array 'crossCurricularRealWorldConnections'".to_string(),
        );
    };
    if xcurr.is_empty() {
        return Err(
            "backwardDesign.crossCurricularRealWorldConnections must not be empty".to_string(),
        );
    }
    if xcurr.iter().any(|item| {
        item.as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    }) {
        return Err(
            "backwardDesign.crossCurricularRealWorldConnections must contain non-empty strings"
                .to_string(),
        );
    }

    let stage3 = backward
        .get("stage3LearningPlan")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "backwardDesign.stage3LearningPlan must be an object".to_string())?;
    let stage3_steps = [
        "hookWarmUp",
        "activatePriorKnowledge",
        "explicitTeachingModeling",
        "guidedPractice",
        "collaborativePractice",
        "independentPractice",
        "closureReflection",
        "exitTicket",
    ];
    for key in stage3_steps {
        if stage3.get(key).is_none() {
            return Err(format!("backwardDesign.stage3LearningPlan missing '{key}'"));
        }
    }

    Ok(())
}

fn auto_mark_missed(conn: &rusqlite::Connection, plan_id: &str) -> Result<(), String> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    conn.execute(
        "UPDATE year_plan_lessons SET status = 'missed', updated_at = ?1 WHERE year_plan_id = ?2 AND status = 'planned' AND teaching_date < ?3 AND is_buffer = 0",
        params![now(), plan_id, today],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn is_detailed_plan_json(value: Option<&str>) -> bool {
    let Some(text) = value else {
        return false;
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    parsed.get("v").and_then(|v| v.as_str()) == Some("1")
        && parsed.get("objective").and_then(|v| v.as_str()).is_some()
        && parsed
            .get("lessonFlow")
            .and_then(|v| v.as_array())
            .is_some()
        && parsed
            .get("backwardDesign")
            .and_then(|v| v.as_object())
            .is_some()
}

fn load_lessons(conn: &rusqlite::Connection, plan_id: &str) -> Result<Vec<YearPlanLesson>, String> {
    auto_mark_missed(conn, plan_id)?;
    let mut stmt = conn
        .prepare("SELECT id, year_plan_id, teaching_date, weekday, sequence_number, curriculum_unit_id, lesson_title, lesson_objective, detailed_plan_attached, coverage_weight, is_buffer, is_holiday_adjusted, status, created_at, updated_at FROM year_plan_lessons WHERE year_plan_id = ?1 ORDER BY sequence_number")
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map(params![plan_id], |row| {
            Ok(YearPlanLesson {
                id: row.get(0)?,
                year_plan_id: row.get(1)?,
                teaching_date: row.get(2)?,
                weekday: row.get(3)?,
                sequence_number: row.get(4)?,
                curriculum_unit_id: row.get(5)?,
                lesson_title: row.get(6)?,
                lesson_objective: row.get(7)?,
                detailed_plan_attached: row.get(8)?,
                coverage_weight: row.get(9)?,
                is_buffer: row.get(10)?,
                is_holiday_adjusted: row.get(11)?,
                status: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

#[tauri::command]
pub async fn update_lesson_status(
    state: DbState<'_>,
    lesson_id: String,
    status: String,
) -> Result<(), String> {
    state.with_conn(|conn| {
        let affected = conn
            .execute(
                "UPDATE year_plan_lessons SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status, now(), lesson_id],
            )
            .map_err(|e| e.to_string())?;
        if affected == 0 {
            return Err("Lesson not found".to_string());
        }
        Ok(())
    })
}

#[tauri::command]
pub async fn reschedule_missed_lessons(
    state: DbState<'_>,
    class_id: String,
) -> Result<Vec<YearPlanLesson>, String> {
    state.with_conn(|conn| {
        let class = get_class(conn, &class_id).map_err(|e| e.to_string())?;
        let plan_id = class
            .year_plan_id
            .ok_or_else(|| "No year plan linked to this class".to_string())?;
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let mut lessons = load_lessons(conn, &plan_id)?;

        let missed: Vec<usize> = lessons
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.is_buffer && l.teaching_date < today && l.status != "completed")
            .map(|(i, _)| i)
            .collect();

        if missed.is_empty() {
            return Ok(lessons);
        }

        let buffers: Vec<usize> = lessons
            .iter()
            .enumerate()
            .filter(|(_, l)| l.is_buffer)
            .map(|(i, _)| i)
            .collect();

        let swaps = missed.len().min(buffers.len());
        let timestamp = now();

        for k in 0..swaps {
            let mi = missed[k];
            let bi = buffers[k];
            let buf_date = lessons[bi].teaching_date.clone();
            let buf_weekday = lessons[bi].weekday.clone();
            let old_date = lessons[mi].teaching_date.clone();
            let old_weekday = lessons[mi].weekday.clone();

            lessons[mi].teaching_date = buf_date.clone();
            lessons[mi].weekday = buf_weekday.clone();
            lessons[mi].status = "planned".to_string();
            lessons[mi].updated_at = timestamp.clone();

            lessons[bi].teaching_date = old_date.clone();
            lessons[bi].weekday = old_weekday.clone();
            lessons[bi].updated_at = timestamp.clone();

            conn.execute(
                "UPDATE year_plan_lessons SET teaching_date = ?1, weekday = ?2, status = ?3, updated_at = ?4 WHERE id = ?5",
                params![buf_date, buf_weekday, "planned", timestamp, lessons[mi].id],
            ).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE year_plan_lessons SET teaching_date = ?1, weekday = ?2, updated_at = ?3 WHERE id = ?4",
                params![old_date, old_weekday, timestamp, lessons[bi].id],
            ).map_err(|e| e.to_string())?;
        }

        lessons.sort_by(|a, b| a.sequence_number.cmp(&b.sequence_number));
        Ok(lessons)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use serde_json::json;

    fn unit(title: &str, lessons: i32, weeks: i32, required: bool) -> CurriculumUnit {
        CurriculumUnit {
            id: format!("unit-{title}"),
            syllabus_id: "syllabus-1".to_string(),
            unit_code: None,
            title: title.to_string(),
            description: None,
            recommended_sequence: 1,
            estimated_lessons: lessons,
            estimated_weeks: weeks,
            assessment_hint: None,
            is_required: required,
            month_label: None,
            schedule_slot_count: 0,
            created_at: now(),
            updated_at: now(),
        }
    }

    fn valid_detailed_plan_json() -> String {
        json!({
            "v": "1",
            "objective": "Students analyze suspense techniques in a short text.",
            "transferGoal": "Apply reading strategies independently.",
            "enduringUnderstanding": "Authors shape emotional response through craft choices.",
            "essentialQuestions": ["How do writers create suspense?"],
            "knowledge": ["Suspense techniques in narrative texts."],
            "skills": ["Annotate textual evidence."],
            "successCriteria": ["I can identify and explain at least two suspense techniques."],
            "assessment": "Exit ticket with text evidence.",
            "performanceTask": "Write a short suspense paragraph using two techniques.",
            "materials": ["Short passage", "Annotation guide"],
            "vocabulary": ["suspense", "foreshadowing"],
            "lessonFlow": [
                {"phase":"Hook","time":"5 min","description":"Quick suspense prompt."},
                {"phase":"Modeling","time":"20 min","description":"Teacher models close reading."},
                {"phase":"Practice","time":"20 min","description":"Students annotate and discuss."}
            ],
            "differentiation": "Provide sentence starters and optional extension prompts.",
            "homework": "Revise suspense paragraph and submit final draft.",
            "crossCurricular": "Connections to media studies and storytelling.",
            "backwardDesign": {
                "lessonInformation": {"class":"8B"},
                "stage1DesiredResults": {"understanding":"Suspense is crafted intentionally."},
                "stage2AssessmentEvidence": {"formative":"Exit ticket"},
                "stage3LearningPlan": {
                    "hookWarmUp": "Two-sentence suspense teaser.",
                    "activatePriorKnowledge": "Recall prior narrative study.",
                    "explicitTeachingModeling": "Model annotation on projector.",
                    "guidedPractice": "Joint annotation of first paragraph.",
                    "collaborativePractice": "Pairs annotate second paragraph.",
                    "independentPractice": "Individual suspense paragraph draft.",
                    "closureReflection": "Share one effective technique used.",
                    "exitTicket": "Name two techniques with examples."
                },
                "differentiation": {"support":"Sentence frames"},
                "resourcesAndMaterials": {"items":["Passage", "Projector"]},
                "timingBreakdown": {"total":"45 minutes"},
                "checkingForUnderstandingQuestions": {"q1":"What clue builds suspense here?"},
                "crossCurricularRealWorldConnections": ["Film trailer pacing", "Podcast storytelling"],
                "classroomManagementConsiderations": {"norms":"Think-pair-share protocols"},
                "homeworkFollowUp": {"task":"Revise paragraph"},
                "teacherReflection": {"prompt":"Which scaffold worked best?"}
            }
        })
        .to_string()
    }

    #[test]
    fn pace_dates_balanced_unchanged() {
        let dates: Vec<NaiveDate> = (1..=10)
            .map(|d| NaiveDate::from_ymd_opt(2026, 1, d).unwrap())
            .collect();
        let result = pace_dates(&dates, Some("balanced"));
        assert_eq!(result.len(), 10);
        assert_eq!(result[0], dates[0]);
    }

    #[test]
    fn pace_dates_front_loaded_unchanged() {
        let dates: Vec<NaiveDate> = (1..=10)
            .map(|d| NaiveDate::from_ymd_opt(2026, 1, d).unwrap())
            .collect();
        let result = pace_dates(&dates, Some("front_loaded"));
        assert_eq!(result, dates);
    }

    #[test]
    fn pace_dates_back_loaded_shifts() {
        let dates: Vec<NaiveDate> = (1..=10)
            .map(|d| NaiveDate::from_ymd_opt(2026, 1, d).unwrap())
            .collect();
        let result = pace_dates(&dates, Some("back_loaded"));
        assert_ne!(result, dates);
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn pace_dates_default_balanced() {
        let dates: Vec<NaiveDate> = (1..=3)
            .map(|d| NaiveDate::from_ymd_opt(2026, 1, d).unwrap())
            .collect();
        let result = pace_dates(&dates, None);
        assert_eq!(result, dates);
    }

    #[test]
    fn requested_lessons_respects_distinct_weeks_estimate() {
        let u = unit("Algebra", 4, 6, true);
        assert_eq!(requested_lessons_for_unit(&u, 2), 12);
    }

    #[test]
    fn duplicated_unit_titles_detects_case_insensitive_duplicates() {
        let units = vec![
            unit("Unit 4 Opening Doors", 4, 4, true),
            unit("unit 4 opening doors", 3, 3, true),
            unit("Geometry", 2, 2, true),
        ];
        let duplicates = duplicated_unit_titles(&units);
        assert_eq!(duplicates, vec!["Unit 4 Opening Doors".to_string()]);
    }

    #[test]
    fn allocator_prioritizes_required_units_when_slots_are_limited() {
        let units = vec![
            unit("Required 1", 3, 3, true),
            unit("Required 2", 3, 3, true),
            unit("Optional", 3, 3, false),
        ];
        let requested = vec![3usize, 3usize, 3usize];
        let (allocated, required_dropped, optional_dropped) =
            allocate_lessons_per_unit(&units, &requested, 2);

        assert_eq!(allocated, vec![1, 1, 0]);
        assert_eq!(required_dropped, 0);
        assert_eq!(optional_dropped, 1);
    }

    #[test]
    fn detailed_json_candidate_strips_control_chars() {
        let raw = "{\n\"objective\":\"bad \u{0000} text\"\n}";
        let parsed = parse_detailed_json_candidate(raw).expect("should parse after sanitization");
        assert_eq!(
            parsed.get("objective").and_then(|v| v.as_str()),
            Some("bad  text")
        );
    }

    #[test]
    fn detailed_json_candidate_quotes_unquoted_keys() {
        let raw = "{objective:\"Focus\",lessonFlow:[]}";
        let parsed = parse_detailed_json_candidate(raw).expect("should quote keys");
        assert_eq!(
            parsed.get("objective").and_then(|v| v.as_str()),
            Some("Focus")
        );
        assert!(parsed.get("lessonFlow").is_some());
    }

    #[test]
    fn detailed_plan_parser_handles_streamed_response_lines() {
        let plan = valid_detailed_plan_json();
        let split = plan.len() / 2;
        let line1 = json!({ "response": "<think>Drafting structureâ€¦</think>" }).to_string();
        let line2 = json!({ "response": &plan[..split], "done": false }).to_string();
        let line3 = json!({ "response": &plan[split..], "done": true }).to_string();
        let raw = format!("{line1}\n{line2}\n{line3}");

        let parsed = parse_and_validate_detailed_plan_value(&raw)
            .expect("should parse streamed JSON fragments");
        assert_eq!(parsed.get("v").and_then(|v| v.as_str()), Some("1"));
        assert!(parsed.get("backwardDesign").is_some());
    }

    #[test]
    fn detailed_plan_parser_handles_reasoning_before_json() {
        let plan = valid_detailed_plan_json();
        let raw = format!("Reasoning draft: {{not_json}}\nFinal answer below:\n{plan}");
        let parsed = parse_and_validate_detailed_plan_value(&raw)
            .expect("should extract final balanced JSON object");
        assert_eq!(parsed.get("v").and_then(|v| v.as_str()), Some("1"));
    }

    #[test]
    fn normalization_repairs_missing_objective_from_backward_design() {
        let mut value: serde_json::Value =
            serde_json::from_str(&valid_detailed_plan_json()).expect("valid base JSON");
        value["objective"] = serde_json::Value::String(String::new());
        value["backwardDesign"]["stage1DesiredResults"]["understanding"] =
            serde_json::Value::String(
                "Writers build suspense through purposeful craft moves.".to_string(),
            );
        normalize_detailed_plan_value(&mut value);
        validate_detailed_plan_json(&value).expect("normalized value should validate");
        assert_eq!(
            value.get("objective").and_then(|v| v.as_str()),
            Some("Writers build suspense through purposeful craft moves.")
        );
    }

    #[test]
    fn sanitize_provider_error_removes_ansi_noise() {
        let raw = "[?2026h[?25l[1G\u{1b}[2KError: 429 Too Many Requests";
        let cleaned = sanitize_provider_error(raw);
        assert!(cleaned.contains("429 Too Many Requests"));
        assert!(!cleaned.contains('\u{1b}'));
    }

    #[test]
    fn fallback_plan_is_valid_detailed_plan_shape() {
        let plan = build_fallback_detailed_plan(
            "Class 8B",
            Some("Reading"),
            "Unit 5 Fear This",
            "Focus on suspense techniques",
            "Lesson 1",
            "2026-09-10",
            1,
            5,
            45,
            "Exit ticket",
        );
        validate_detailed_plan_json(&plan).expect("fallback plan should satisfy validator");
        assert_eq!(plan.get("v").and_then(|v| v.as_str()), Some("1"));
    }

    #[test]
    fn cloud_model_detection_identifies_cloud_suffixes() {
        let cloud_cfg = crate::services::llm_service::LocalModelConfig {
            provider: "ollama".to_string(),
            model: "deepseek-v3.1:671b-cloud".to_string(),
            available: true,
            detail: String::new(),
        };
        let local_cfg = crate::services::llm_service::LocalModelConfig {
            provider: "ollama".to_string(),
            model: "llama3.1:latest".to_string(),
            available: true,
            detail: String::new(),
        };
        assert!(is_cloud_backed_model(&cloud_cfg));
        assert!(!is_cloud_backed_model(&local_cfg));
    }

    #[test]
    fn cloud_cooldown_is_nonzero_for_cloud_models() {
        let cloud_cfg = crate::services::llm_service::LocalModelConfig {
            provider: "ollama".to_string(),
            model: "minimax-m2.5:cloud".to_string(),
            available: true,
            detail: String::new(),
        };
        let local_cfg = crate::services::llm_service::LocalModelConfig {
            provider: "ollama".to_string(),
            model: "qwen3.5:4b".to_string(),
            available: true,
            detail: String::new(),
        };
        assert!(cloud_generation_cooldown(&cloud_cfg) >= Duration::from_millis(1_000));
        assert_eq!(
            cloud_generation_cooldown(&local_cfg),
            Duration::from_millis(0)
        );
    }
}
