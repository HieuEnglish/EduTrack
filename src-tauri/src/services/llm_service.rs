use crate::services::hierarchy::{new_id, now, CurriculumUnit, DbState};
use crate::services::jobs::{complete_job, create_job, fail_job, mark_job_running};
use crate::services::syllabus_processing::load_units;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Output};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelConfig {
    pub provider: String,
    pub model: String,
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyllabusExtractionDiagnostics {
    pub confidence: i32,
    pub detected_layout: String,
    pub unit_count: i32,
    pub notes: Vec<String>,
    pub suggested_actions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelUnit {
    title: String,
    description: Option<String>,
    #[serde(alias = "estimated_lessons")]
    estimated_lessons: Option<i32>,
    #[serde(alias = "assessment_hint")]
    assessment_hint: Option<String>,
    #[serde(default, alias = "month_label")]
    month_label: Option<String>,
}

pub fn extract_units_advanced(text: &str, syllabus_id: &str) -> Vec<CurriculumUnit> {
    let mut candidates = extract_semester_timeline_units(text);
    if candidates.len() < 2 {
        candidates = extract_by_headings(text);
    }
    if candidates.len() < 2 {
        candidates = extract_by_numbered_lines(text);
    }
    if candidates.len() < 2 {
        candidates = extract_by_paragraph_chunks(text);
    }

    candidates
        .into_iter()
        .enumerate()
        .map(|(index, (title, body))| {
            let estimated_lessons = estimate_lessons(&title, &body);
            CurriculumUnit {
                id: new_id("unit"),
                syllabus_id: syllabus_id.to_string(),
                unit_code: Some(format!("UNIT-{:03}", index + 1)),
                title: clean_title(&title),
                description: Some(body.trim().chars().take(900).collect::<String>()),
                recommended_sequence: (index + 1) as i32,
                estimated_lessons,
                estimated_weeks: estimated_lessons.max(1),
                assessment_hint: infer_assessment_hint(&body),
                is_required: !body.to_ascii_lowercase().contains("optional"),
                month_label: None,
                schedule_slot_count: 0,
                created_at: now(),
                updated_at: now(),
            }
        })
        .collect()
}

fn extract_semester_timeline_units(text: &str) -> Vec<(String, String)> {
    let month_line = Regex::new(
        r"(?i)^\s*(January|February|March|April|May|June|July|August|September|October|November|December)(?:\s*[-–]\s*(January|February|March|April|May|June|July|August|September|October|November|December))?\s*$",
    )
    .expect("valid month line regex");
    let unit_line = Regex::new(r"(?i)^\s*Unit\s*(\d{1,2})\s*[:.\-]?\s*(.+?)\s*$")
        .expect("valid unit line regex");
    let continuation_suffix =
        Regex::new(r"(?i)\s*\((?:continuation|continued)\)\s*$").expect("valid continuation regex");

    let section_header = Regex::new(r"^\s*(?:I{1,3}|IV|V|VI{1,3}|IX|X)\s*\..+\s*$")
        .expect("valid section header regex");

    let mut current_month = String::new();
    let mut order: Vec<String> = Vec::new();
    let mut buckets: HashMap<String, (String, Vec<String>)> = HashMap::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("semester ") {
            continue;
        }
        if month_line.is_match(line) {
            current_month = line.replace('–', "-");
            continue;
        }
        if current_month.is_empty() {
            continue;
        }
        if section_header.is_match(line) {
            current_month.clear();
            continue;
        }
        let Some(caps) = unit_line.captures(line) else {
            continue;
        };
        let number = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        let raw_title = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
        let base_title = continuation_suffix
            .replace(raw_title, "")
            .trim()
            .trim_matches(['-', ':', '.'])
            .to_string();
        if base_title.is_empty() {
            continue;
        }
        let key = format!(
            "{}:{}",
            number,
            base_title
                .to_ascii_lowercase()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        );
        if !buckets.contains_key(&key) {
            order.push(key.clone());
            buckets.insert(
                key.clone(),
                (format!("Unit {}: {}", number, base_title), Vec::new()),
            );
        }
        if !current_month.is_empty() {
            let entry = buckets.get_mut(&key).expect("bucket exists");
            if !entry
                .1
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&current_month))
            {
                entry.1.push(current_month.clone());
            }
        }
    }

    if order.len() < 3 {
        return Vec::new();
    }

    order
        .into_iter()
        .filter_map(|key| buckets.remove(&key))
        .map(|(title, months)| {
            let body = if months.is_empty() {
                "Semester timeline detected, but no month labels were parsed.".to_string()
            } else {
                format!("Assigned timeline: {}.", months.join(", "))
            };
            (title, body)
        })
        .collect()
}

fn extract_by_headings(text: &str) -> Vec<(String, String)> {
    let heading = Regex::new(
        r"(?im)^\s*((?:unit|module|chapter|strand|topic|term|unidad|modulo|mï¿½dulo|capitulo|capï¿½tulo|tema|lecciï¿½n|leccion|tï¿½pico|topico|unidade|leï¿½on|sujet|thï¿½me|asignatura)\s+\d+[A-Za-z]?\s*(?:[:.\-ï¿½ï¿½)]\s*)?.{2,120})\s*$",
    )
    .expect("valid heading regex");
    let matches: Vec<_> = heading.find_iter(text).collect();
    matches
        .iter()
        .enumerate()
        .map(|(idx, m)| {
            let end = matches.get(idx + 1).map_or(text.len(), |next| next.start());
            let title = m.as_str().trim().to_string();
            let body = text[m.end()..end].trim().to_string();
            // PDF extraction can produce heading-only unit lists with no body between headings.
            // Keep those units by falling back to the heading text.
            let body = if body.is_empty() { title.clone() } else { body };
            (title, body)
        })
        .collect()
}

fn extract_by_numbered_lines(text: &str) -> Vec<(String, String)> {
    let numbered = Regex::new(r"(?m)^\s*(\d{1,2}[.)]\s+.{4,100})$").expect("valid numbered regex");
    let matches: Vec<_> = numbered.find_iter(text).collect();
    matches
        .iter()
        .enumerate()
        .map(|(idx, m)| {
            let end = matches.get(idx + 1).map_or(text.len(), |next| next.start());
            (
                m.as_str().to_string(),
                text[m.end()..end].trim().to_string(),
            )
        })
        .filter(|(_, body)| body.split_whitespace().count() > 8)
        .collect()
}

fn extract_by_paragraph_chunks(text: &str) -> Vec<(String, String)> {
    let paragraphs: Vec<_> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| p.split_whitespace().count() > 12)
        .collect();
    if paragraphs.is_empty() {
        return vec![(
            "Full Syllabus Coverage".to_string(),
            text.trim().to_string(),
        )];
    }
    let chunk_size = (paragraphs.len() / 6).max(1);
    paragraphs
        .chunks(chunk_size)
        .take(10)
        .enumerate()
        .map(|(idx, chunk)| {
            let body = chunk.join("\n\n");
            let title = body
                .lines()
                .next()
                .unwrap_or("Instructional Unit")
                .chars()
                .take(80)
                .collect::<String>();
            (format!("Unit {}: {title}", idx + 1), body)
        })
        .collect()
}

fn clean_title(title: &str) -> String {
    title
        .trim()
        .trim_matches(['-', ':', '.'])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn estimate_lessons(title: &str, body: &str) -> i32 {
    let re = Regex::new(r"(?i)(\d{1,2})\s+(lessons?|weeks?|wks?|sessions?)")
        .expect("valid lesson estimate regex");
    let from_title = re
        .captures(title)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .filter(|&n| n <= 12);
    if let Some(count) = from_title {
        return count;
    }
    let from_body = re
        .captures_iter(body)
        .filter_map(|caps| caps.get(1))
        .filter_map(|m| m.as_str().parse::<i32>().ok())
        .filter(|&n| n <= 12)
        .max();
    if let Some(count) = from_body {
        return count;
    }

    let month_re = Regex::new(
        r"(?i)(?:January|February|March|April|May|June|July|August|September|October|November|December)",
    )
    .expect("valid month regex");
    let combined = format!("{title}\n{body}");
    let month_count = month_re.find_iter(&combined).count();
    if month_count > 0 {
        return ((month_count as i32) * 4).clamp(4, 24);
    }

    4
}

fn infer_assessment_hint(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    if lower.contains("project") {
        Some("Project or performance task".to_string())
    } else if lower.contains("exam") || lower.contains("test") {
        Some("End-of-unit test".to_string())
    } else if lower.contains("quiz") {
        Some("Short formative quiz".to_string())
    } else if lower.contains("presentation") {
        Some("Student presentation".to_string())
    } else {
        Some("Teacher-reviewed formative assessment".to_string())
    }
}

pub(crate) fn extract_units_with_provider(
    config: &LocalModelConfig,
    text: &str,
    syllabus_id: &str,
) -> Result<Vec<CurriculumUnit>, String> {
    let prompt = format!(
        "Extract the teaching schedule from this syllabus. Return ONLY JSON {{\"units\":[...]}}. Each unit: title, description, estimatedLessons (default 4), assessmentHint, monthLabel (the assigned month/period).\n\
Rules: (1) Extract every schedule entry including continuations (each gets its own entry). (2) Skip objective/outcome-only listings. (3) Order chronologically by earliest time period.\n\
Syllabus:\n{}",
        text.chars().take(14_000).collect::<String>()
    );
    let output = run_provider_prompt(config, &prompt)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    let response_text = if config.provider == "opencode" {
        let extracted = extract_text_from_jsonl_stream(&raw);
        match extracted {
            Some(text) => text,
            None => {
                let error_msg = extract_first_jsonl_error(&raw);
                if let Some(msg) = error_msg {
                    return Err(format!("OpenCode provider error: {msg}"));
                }
                raw.trim().to_string()
            }
        }
    } else {
        raw.to_string()
    };
    if response_text.trim().is_empty() {
        return Err("model returned empty response".to_string());
    }
    let model_units = parse_model_units(&response_text)?;
    if model_units.is_empty() {
        return Err("local model returned no units".to_string());
    }
    Ok(build_curriculum_units(model_units, text, syllabus_id))
}

fn build_curriculum_units(
    model_units: Vec<ModelUnit>,
    text: &str,
    syllabus_id: &str,
) -> Vec<CurriculumUnit> {
    let schedule_slot_count = model_units.len() as i32;
    let provider_units: Vec<CurriculumUnit> = model_units
        .into_iter()
        .enumerate()
        .map(|(index, unit)| {
            let estimated_lessons = unit.estimated_lessons.unwrap_or(4).clamp(1, 24);
            CurriculumUnit {
                id: new_id("unit"),
                syllabus_id: syllabus_id.to_string(),
                unit_code: Some(format!("UNIT-{:03}", index + 1)),
                title: unit.title,
                description: unit.description,
                recommended_sequence: (index + 1) as i32,
                estimated_lessons,
                estimated_weeks: estimated_lessons.max(1),
                assessment_hint: unit.assessment_hint,
                is_required: true,
                month_label: unit.month_label,
                schedule_slot_count,
                created_at: now(),
                updated_at: now(),
            }
        })
        .collect();

    let provider_final = finalize_curriculum_units(provider_units, text, syllabus_id);
    let heuristic_final =
        finalize_curriculum_units(extract_units_advanced(text, syllabus_id), text, syllabus_id);

    if should_prefer_heuristic_units(&provider_final, &heuristic_final, text) {
        heuristic_final
    } else {
        provider_final
    }
}

fn finalize_curriculum_units(
    units: Vec<CurriculumUnit>,
    text: &str,
    syllabus_id: &str,
) -> Vec<CurriculumUnit> {
    let normalized = normalize_extracted_units(units, text, syllabus_id);
    enrich_units_with_source_timeline(normalized, text)
}

fn should_prefer_heuristic_units(
    provider_units: &[CurriculumUnit],
    heuristic_units: &[CurriculumUnit],
    text: &str,
) -> bool {
    let provider_count = provider_units.len();
    let heuristic_count = heuristic_units.len();
    if heuristic_count < 2 || heuristic_count <= provider_count {
        return false;
    }
    if provider_count <= 1 {
        return true;
    }
    if heuristic_count >= provider_count.saturating_mul(2) {
        return true;
    }
    let heading_count = Regex::new(r"(?im)^\s*unit\s+\d+")
        .expect("valid unit heading regex")
        .find_iter(text)
        .count();
    heading_count >= heuristic_count && heuristic_count >= provider_count + 2
}

fn merge_timeline_labels(base: Option<&str>, extra: Option<&str>) -> Option<String> {
    let mut values: Vec<String> = Vec::new();
    for raw in [base, extra].into_iter().flatten() {
        for segment in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if !values
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(segment))
            {
                values.push(segment.to_string());
            }
        }
    }
    if values.is_empty() {
        None
    } else {
        Some(values.join(", "))
    }
}

fn title_tokens_for_match(title: &str) -> Vec<String> {
    let core = topic_core_from_title(title);
    core.split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
        .filter(|token| token.len() >= 3)
        .filter(|token| {
            !matches!(
                *token,
                "unit"
                    | "lesson"
                    | "lessons"
                    | "module"
                    | "topic"
                    | "the"
                    | "and"
                    | "for"
                    | "with"
                    | "continuation"
                    | "continued"
            )
        })
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn title_match_score(a: &str, b: &str) -> i32 {
    let left = title_tokens_for_match(a);
    let right = title_tokens_for_match(b);
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    let mut overlap = 0;
    for token in &left {
        if right.iter().any(|candidate| candidate == token) {
            overlap += 1;
        }
    }
    overlap
}

fn enrich_units_with_source_timeline(
    mut units: Vec<CurriculumUnit>,
    text: &str,
) -> Vec<CurriculumUnit> {
    let extracted = extract_semester_timeline_units(text);
    if extracted.is_empty() {
        return units;
    }
    let mut months_by_unit_number: HashMap<String, String> = HashMap::new();
    let mut months_by_source_title: Vec<(String, String)> = Vec::new();
    for (title, body) in extracted {
        let Some((unit_number, _)) = canonical_unit_key(&title) else {
            continue;
        };
        let Some(label) = assigned_timeline_label_from_body(&body) else {
            continue;
        };
        if label.is_empty() {
            continue;
        }
        match months_by_unit_number.get_mut(&unit_number) {
            Some(existing) => {
                for segment in label.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    if !existing
                        .split(',')
                        .map(str::trim)
                        .any(|value| value.eq_ignore_ascii_case(segment))
                    {
                        existing.push_str(", ");
                        existing.push_str(segment);
                    }
                }
            }
            None => {
                months_by_unit_number.insert(unit_number, label);
            }
        }
        months_by_source_title.push((title, body));
    }
    for unit in &mut units {
        let unit_number = canonical_unit_key(&unit.title).map(|(n, _)| n);
        if let Some(number) = unit_number {
            if let Some(label) = months_by_unit_number.get(&number) {
                unit.month_label = merge_timeline_labels(unit.month_label.as_deref(), Some(label));
                continue;
            }
        }

        let mut best_label: Option<String> = None;
        let mut best_score = 0i32;
        for (source_title, source_body) in &months_by_source_title {
            let score = title_match_score(&unit.title, source_title);
            if score > best_score {
                let Some(label) = assigned_timeline_label_from_body(source_body) else {
                    continue;
                };
                best_score = score;
                best_label = Some(label);
            }
        }
        if best_score >= 2 {
            unit.month_label =
                merge_timeline_labels(unit.month_label.as_deref(), best_label.as_deref());
        }
    }
    units
}

fn assigned_timeline_label_from_body(body: &str) -> Option<String> {
    let value = body.trim();
    let prefix = "assigned timeline:";
    if value.len() < prefix.len() || !value[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    let label = value[prefix.len()..].trim().trim_end_matches('.').trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

pub(crate) fn normalize_extracted_units(
    units: Vec<CurriculumUnit>,
    text: &str,
    syllabus_id: &str,
) -> Vec<CurriculumUnit> {
    let timeline_layout = detect_semester_timeline_layout(text);
    let blocked_title = Regex::new(
        r"(?i)^(at the end of the unit|students will be able to|learning objective|success criteria)",
    )
    .expect("valid blocked-title regex");
    let trailing_continuation = Regex::new(r"(?i)\s*\((?:continuation|continued)\)\s*$")
        .expect("valid continuation suffix regex");
    let mut merged: Vec<CurriculumUnit> = Vec::new();
    let mut key_to_index: HashMap<String, usize> = HashMap::new();

    for unit in units {
        let raw_title = clean_title(&unit.title);
        if raw_title.is_empty() || blocked_title.is_match(raw_title.as_str()) {
            continue;
        }
        let normalized_title = trailing_continuation
            .replace(&raw_title, "")
            .trim()
            .to_string();
        if normalized_title.is_empty() {
            continue;
        }

        let canonical = canonical_unit_key(&normalized_title);
        let merge_key = if timeline_layout {
            canonical
                .as_ref()
                .map(|(n, _)| format!("unit-no:{n}"))
                .unwrap_or_else(|| format!("title:{}", normalized_title.to_ascii_lowercase()))
        } else {
            canonical
                .as_ref()
                .map(|(n, t)| format!("unit:{n}:{t}"))
                .unwrap_or_else(|| format!("title:{}", normalized_title.to_ascii_lowercase()))
        };

        if let Some(existing_idx) = key_to_index.get(&merge_key).copied() {
            let existing = merged.get_mut(existing_idx).expect("valid merged index");
            if let Some(desc) = unit
                .description
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let existing_desc = existing.description.clone().unwrap_or_default();
                if !existing_desc
                    .to_ascii_lowercase()
                    .contains(&desc.to_ascii_lowercase())
                {
                    let combined = if existing_desc.is_empty() {
                        desc.to_string()
                    } else {
                        format!("{existing_desc}\n{desc}")
                    };
                    existing.description = Some(combined.chars().take(900).collect());
                }
            }
            existing.estimated_lessons = existing.estimated_lessons.max(unit.estimated_lessons);
            existing.estimated_weeks = existing.estimated_weeks.max(unit.estimated_weeks);
            existing.month_label =
                merge_timeline_labels(existing.month_label.as_deref(), unit.month_label.as_deref());
            continue;
        }

        let mut clean_unit = unit;
        clean_unit.title = normalized_title;
        clean_unit.syllabus_id = syllabus_id.to_string();
        clean_unit.description = clean_unit
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.chars().take(900).collect::<String>());
        key_to_index.insert(merge_key, merged.len());
        merged.push(clean_unit);
    }

    for (index, unit) in merged.iter_mut().enumerate() {
        unit.recommended_sequence = (index + 1) as i32;
        unit.unit_code = Some(format!("UNIT-{:03}", index + 1));
        if unit.estimated_lessons <= 0 {
            unit.estimated_lessons = 4;
        }
        if unit.estimated_weeks <= 0 {
            unit.estimated_weeks = unit.estimated_lessons.max(1);
        }
    }

    prune_derivative_and_unsupported_units(merged, text, timeline_layout)
}

fn prune_derivative_and_unsupported_units(
    units: Vec<CurriculumUnit>,
    text: &str,
    timeline_layout: bool,
) -> Vec<CurriculumUnit> {
    if units.is_empty() {
        return units;
    }
    let mut topic_has_non_write: HashMap<String, bool> = HashMap::new();
    for unit in &units {
        let core = topic_core_from_title(&unit.title);
        if core.is_empty() {
            continue;
        }
        if !is_write_variant_title(&unit.title) {
            topic_has_non_write.insert(core, true);
        } else {
            topic_has_non_write.entry(core).or_insert(false);
        }
    }

    let mut filtered = Vec::new();
    for unit in units {
        let core = topic_core_from_title(&unit.title);
        let is_write_variant = is_write_variant_title(&unit.title);
        if is_write_variant && topic_has_non_write.get(&core).copied().unwrap_or(false) {
            continue;
        }
        if timeline_layout && !title_has_source_evidence(&unit.title, text) {
            continue;
        }
        filtered.push(unit);
    }
    if filtered.is_empty() {
        return Vec::new();
    }
    for (index, unit) in filtered.iter_mut().enumerate() {
        unit.recommended_sequence = (index + 1) as i32;
        unit.unit_code = Some(format!("UNIT-{:03}", index + 1));
    }
    filtered
}

fn is_write_variant_title(title: &str) -> bool {
    let normalized = strip_unit_prefix(title).to_ascii_lowercase();
    normalized.starts_with("write ")
        || normalized.starts_with("writing ")
        || normalized.starts_with("to write ")
}

fn topic_core_from_title(title: &str) -> String {
    let mut value = strip_unit_prefix(title);
    let continuation_suffix = Regex::new(r"(?i)\s*\((?:continuation|continued)\)\s*$")
        .expect("valid continuation suffix regex");
    value = continuation_suffix.replace(&value, "").to_string();
    let lower = value.to_ascii_lowercase();
    let stripped = if lower.starts_with("write a ") {
        value[8..].to_string()
    } else if lower.starts_with("write an ") {
        value[9..].to_string()
    } else if lower.starts_with("write the ") {
        value[10..].to_string()
    } else if lower.starts_with("write ") {
        value[6..].to_string()
    } else if lower.starts_with("writing a ") {
        value[10..].to_string()
    } else if lower.starts_with("writing an ") {
        value[11..].to_string()
    } else if lower.starts_with("writing ") {
        value[8..].to_string()
    } else {
        value
    };
    stripped
        .trim()
        .trim_matches(['-', ':', '.'])
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_unit_prefix(title: &str) -> String {
    let re = Regex::new(r"(?i)^\s*unit\s*\d{1,2}\s*[:.\-]?\s*").expect("valid unit prefix regex");
    re.replace(title, "").trim().to_string()
}

fn title_has_source_evidence(title: &str, text: &str) -> bool {
    let source = text.to_ascii_lowercase();
    let core = topic_core_from_title(title);
    if core.is_empty() {
        return false;
    }
    if source.contains(&core) {
        return true;
    }
    let tokens: Vec<&str> = core
        .split_whitespace()
        .filter(|token| token.len() >= 4)
        .filter(|token| {
            !matches!(
                *token,
                "essay" | "unit" | "lesson" | "lessons" | "module" | "topic"
            )
        })
        .collect();
    if tokens.is_empty() {
        return false;
    }
    let hit_count = tokens
        .iter()
        .filter(|token| source.contains(**token))
        .count();
    hit_count >= 2 || (tokens.len() == 1 && hit_count == 1)
}

fn canonical_unit_key(title: &str) -> Option<(String, String)> {
    let unit_re = Regex::new(r"(?i)^\s*unit\s*(\d{1,2})\s*[:.\-]?\s*(.+?)\s*$")
        .expect("valid canonical unit regex");
    let caps = unit_re.captures(title)?;
    let number = caps.get(1)?.as_str().to_string();
    let name = caps
        .get(2)?
        .as_str()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Some((number, name))
}

fn detect_semester_timeline_layout(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("semester") {
        return false;
    }
    let month_line = Regex::new(
        r"(?im)^\s*(January|February|March|April|May|June|July|August|September|October|November|December)(?:\s*[-–]\s*(January|February|March|April|May|June|July|August|September|October|November|December))?\s*$",
    )
    .expect("valid month layout regex");
    let unit_line =
        Regex::new(r"(?im)^\s*Unit\s*\d{1,2}\s*[:.\-]?").expect("valid unit layout regex");
    let month_lines = month_line.find_iter(text).count();
    let unit_lines = unit_line.find_iter(text).count();
    month_lines >= 4 && unit_lines >= 4
}

fn parse_model_units(raw: &str) -> Result<Vec<ModelUnit>, String> {
    let candidates = collect_json_candidates(raw);
    if candidates.is_empty() {
        return Err("local model did not return JSON".to_string());
    }
    let mut errors = Vec::new();
    for candidate in candidates {
        match try_decode_units_text(&candidate) {
            Ok(units) => return Ok(units),
            Err(err) => errors.push(err),
        }
    }
    if errors.len() == 1 {
        Err(errors.remove(0))
    } else {
        Err(errors.join("; alternate JSON parse also failed: "))
    }
}

fn try_decode_units_text(snippet: &str) -> Result<Vec<ModelUnit>, String> {
    decode_units_json(snippet).or_else(|initial_err| {
        let cleaned = strip_control_chars(snippet);
        decode_units_json(&cleaned).or_else(|retry_err| {
            let trailing_fixed = strip_trailing_commas(&cleaned);
            decode_units_json(&trailing_fixed).map_err(|final_err| {
                format!(
                    "{final_err} (after sanitizing control characters; original parse error: {initial_err}; sanitized parse error: {retry_err})"
                )
            })
        })
    })
}

fn collect_json_candidates(raw: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let trimmed = raw.trim();
    if !trimmed.is_empty() {
        push_candidate(&mut candidates, trimmed.to_string());
    }

    let object_span = raw
        .find('{')
        .and_then(|start| raw.rfind('}').map(|end| (start, end)));
    let array_span = raw
        .find('[')
        .and_then(|start| raw.rfind(']').map(|end| (start, end)));
    match (object_span, array_span) {
        (Some(obj), Some(arr)) if arr.0 < obj.0 => {
            push_candidate(&mut candidates, raw[arr.0..=arr.1].to_string());
            push_candidate(&mut candidates, raw[obj.0..=obj.1].to_string());
        }
        (Some(obj), Some(arr)) => {
            push_candidate(&mut candidates, raw[obj.0..=obj.1].to_string());
            push_candidate(&mut candidates, raw[arr.0..=arr.1].to_string());
        }
        (Some(obj), None) => push_candidate(&mut candidates, raw[obj.0..=obj.1].to_string()),
        (None, Some(arr)) => push_candidate(&mut candidates, raw[arr.0..=arr.1].to_string()),
        (None, None) => {}
    }

    for fenced in extract_fenced_candidates(raw) {
        push_candidate(&mut candidates, fenced);
    }

    if let Some(units_obj) = extract_units_object_candidate(raw) {
        push_candidate(&mut candidates, units_obj);
    }

    candidates
}

fn push_candidate(candidates: &mut Vec<String>, snippet: String) {
    let normalized = snippet.trim();
    if normalized.is_empty() {
        return;
    }
    if !candidates.iter().any(|existing| existing == normalized) {
        candidates.push(normalized.to_string());
    }
}

fn extract_fenced_candidates(raw: &str) -> Vec<String> {
    let fenced = Regex::new(r"(?s)```(?:json)?\s*(.*?)\s*```").expect("valid fenced-json regex");
    let mut snippets = Vec::new();
    for cap in fenced.captures_iter(raw) {
        let Some(block) = cap.get(1).map(|m| m.as_str()) else {
            continue;
        };
        let object_span = block
            .find('{')
            .and_then(|start| block.rfind('}').map(|end| (start, end)));
        let array_span = block
            .find('[')
            .and_then(|start| block.rfind(']').map(|end| (start, end)));
        match (object_span, array_span) {
            (Some(obj), Some(arr)) if arr.0 < obj.0 => {
                snippets.push(block[arr.0..=arr.1].to_string());
                snippets.push(block[obj.0..=obj.1].to_string());
            }
            (Some(obj), Some(arr)) => {
                snippets.push(block[obj.0..=obj.1].to_string());
                snippets.push(block[arr.0..=arr.1].to_string());
            }
            (Some(obj), None) => snippets.push(block[obj.0..=obj.1].to_string()),
            (None, Some(arr)) => snippets.push(block[arr.0..=arr.1].to_string()),
            (None, None) => snippets.push(block.to_string()),
        }
    }
    snippets
}

fn extract_units_object_candidate(raw: &str) -> Option<String> {
    let units_key_pos = raw.find("\"units\"").or_else(|| raw.find("'units'"))?;
    let bracket_start_rel = raw[units_key_pos..].find('[')?;
    let bracket_start = units_key_pos + bracket_start_rel;
    let bracket_end = find_matching_bracket(raw, bracket_start, '[', ']')?;
    let arr = &raw[bracket_start..=bracket_end];
    Some(format!(r#"{{"units":{arr}}}"#))
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

fn decode_units_json(text: &str) -> Result<Vec<ModelUnit>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if let Some(units_value) = value.get("units") {
        serde_json::from_value(units_value.clone()).map_err(|e| e.to_string())
    } else if value.is_array() {
        serde_json::from_value(value).map_err(|e| e.to_string())
    } else {
        Err("JSON must contain a 'units' array".to_string())
    }
}

fn strip_control_chars(text: &str) -> String {
    text.chars().filter(|c| *c >= ' ').collect()
}

fn strip_trailing_commas(text: &str) -> String {
    let re = Regex::new(r",\s*([}\]])").expect("valid trailing comma regex");
    re.replace_all(text, "$1").to_string()
}

pub fn generate_local_report(config: &LocalModelConfig, prompt: &str) -> Result<String, String> {
    let output = run_provider_prompt(config, prompt)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "provider command failed with no error output".to_string()
        };
        return Err(message);
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let mut text = stdout.trim().to_string();
    if text.is_empty() {
        text = stderr.trim().to_string();
    }
    if config.provider == "opencode" {
        if let Some(extracted) = extract_text_from_jsonl_stream(&text) {
            return Ok(extracted);
        }
    }
    Ok(text)
}

fn run_provider_prompt(
    config: &LocalModelConfig,
    prompt: &str,
) -> Result<std::process::Output, String> {
    validate_model_name(&config.model)?;
    match config.provider.as_str() {
        "opencode" => {
            let mut command = opencode_command();
            command.env("NO_COLOR", "1").env("TERM", "dumb");
            command.args(["run", "--format", "json", "--pure"]);
            if !config.model.trim().is_empty() {
                command.args(["--model", &config.model]);
            }
            command
                .arg(prompt)
                .output()
                .map_err(|e| format!("failed to start OpenCode CLI: {e}"))
        }
        "ollama" | "" => {
            let mut command = Command::new("ollama");
            command
                .env("NO_COLOR", "1")
                .env("TERM", "dumb")
                .args(["run", &config.model, prompt]);
            command
                .output()
                .map_err(|e| format!("failed to start Ollama CLI: {e}"))
        }
        other => Err(format!("unsupported LLM provider '{other}'")),
    }
}

fn extract_first_jsonl_error(raw: &str) -> Option<String> {
    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(|t| t.as_str()) == Some("error") {
            if let Some(data) = value.get("error").and_then(|e| e.get("data")) {
                if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                    return Some(msg.to_string());
                }
                if let Some(name) = value
                    .get("error")
                    .and_then(|e| e.get("name"))
                    .and_then(|n| n.as_str())
                {
                    return Some(name.to_string());
                }
            }
            return Some("unknown error from provider".to_string());
        }
    }
    None
}

fn extract_text_from_jsonl_stream(raw: &str) -> Option<String> {
    let mut combined = String::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        append_json_text_field(&mut combined, Some(&value));
    }
    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        append_json_text_field(&mut combined, value.get("response"));
        append_json_text_field(&mut combined, value.get("text"));
        append_json_text_field(&mut combined, value.get("content"));
        append_json_text_field(
            &mut combined,
            value.get("message").and_then(|v| v.get("content")),
        );
        append_json_text_field(&mut combined, value.get("part").and_then(|v| v.get("text")));
        append_json_text_field(
            &mut combined,
            value.get("part").and_then(|v| v.get("content")),
        );
    }
    let cleaned = combined.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn append_json_text_field(target: &mut String, value: Option<&serde_json::Value>) {
    let Some(value) = value else {
        return;
    };
    if let Some(text) = value.as_str() {
        target.push_str(text);
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            append_json_text_field(target, Some(item));
        }
        return;
    }
    if let Some(obj) = value.as_object() {
        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
            target.push_str(text);
            return;
        }
        if let Some(text) = obj.get("content").and_then(|v| v.as_str()) {
            target.push_str(text);
        }
    }
}

fn validate_model_name(model: &str) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("model name cannot be empty".to_string());
    }
    if model.len() > 200 {
        return Err("model name too long (max 200 characters)".to_string());
    }
    if model.contains(['\0', '\n', '\r', '|', ';', '&', '$']) {
        return Err("model name contains invalid characters".to_string());
    }
    Ok(())
}

fn config_from_env() -> LocalModelConfig {
    let provider = std::env::var("EDUTRACK_LLM_PROVIDER").unwrap_or_else(|_| "ollama".to_string());
    let model = match provider.as_str() {
        "opencode" => std::env::var("EDUTRACK_OPENCODE_MODEL")
            .or_else(|_| std::env::var("EDUTRACK_LLM_MODEL"))
            .unwrap_or_else(|_| "anthropic/claude-sonnet-4-5".to_string()),
        _ => std::env::var("EDUTRACK_OLLAMA_MODEL")
            .or_else(|_| std::env::var("EDUTRACK_LLM_MODEL"))
            .unwrap_or_else(|_| "llama3.1".to_string()),
    };
    LocalModelConfig {
        provider,
        model,
        available: false,
        detail: "not probed".to_string(),
    }
}

pub fn load_llm_config(conn: &rusqlite::Connection) -> LocalModelConfig {
    let mut config = config_from_env();
    if let Ok(provider) = conn.query_row(
        "SELECT value_json FROM settings WHERE key = 'llm.provider'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        config.provider = serde_json::from_str::<String>(&provider).unwrap_or(provider);
    }
    if let Ok(model) = conn.query_row(
        "SELECT value_json FROM settings WHERE key = 'llm.model'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        config.model = serde_json::from_str::<String>(&model).unwrap_or(model);
    }
    config
}

pub fn load_and_probe_llm_config(conn: &rusqlite::Connection) -> LocalModelConfig {
    probe_provider(load_llm_config(conn))
}

fn probe_provider(mut config: LocalModelConfig) -> LocalModelConfig {
    let probe = match config.provider.as_str() {
        "opencode" => run_opencode(&["--version"]),
        _ => Command::new("ollama")
            .arg("--version")
            .output()
            .map_err(|e| e.to_string()),
    };
    match probe {
        Ok(output) if output.status.success() => {
            config.available = true;
            config.detail = String::from_utf8_lossy(&output.stdout).trim().to_string();
            config
        }
        Ok(output) => {
            config.available = false;
            config.detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            config
        }
        Err(err) => {
            config.available = false;
            config.detail = err.to_string();
            config
        }
    }
}

#[tauri::command]
pub async fn get_local_model_config(state: DbState<'_>) -> Result<LocalModelConfig, String> {
    state.with_conn(|conn| Ok(probe_provider(load_llm_config(conn))))
}

#[tauri::command]
pub async fn set_llm_provider(
    state: DbState<'_>,
    provider: String,
    model: String,
) -> Result<LocalModelConfig, String> {
    let normalized = provider.to_ascii_lowercase();
    if !matches!(normalized.as_str(), "ollama" | "opencode") {
        return Err("provider must be 'ollama' or 'opencode'".to_string());
    }
    state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES ('llm.provider', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![serde_json::to_string(&normalized).map_err(|e| e.to_string())?, now()],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES ('llm.model', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![serde_json::to_string(&model).map_err(|e| e.to_string())?, now()],
        )
        .map_err(|e| e.to_string())?;
        Ok(probe_provider(LocalModelConfig {
            provider: normalized,
            model,
            available: true,
            detail: "not probed".to_string(),
        }))
    })
}

#[tauri::command]
pub async fn list_llm_models(
    state: DbState<'_>,
    provider: String,
) -> Result<Vec<LlmModelOption>, String> {
    let normalized = provider.to_ascii_lowercase();
    state.with_conn(|conn| {
        let configured = load_llm_config(conn);
        let models = match normalized.as_str() {
            "opencode" => list_opencode_models().unwrap_or_else(|_| fallback_models("opencode")),
            "ollama" => list_ollama_models().unwrap_or_else(|_| fallback_models("ollama")),
            _ => return Err("provider must be 'ollama' or 'opencode'".to_string()),
        };
        let mut merged = models;
        if configured.provider == normalized
            && !configured.model.trim().is_empty()
            && !merged.iter().any(|model| model.id == configured.model)
        {
            merged.insert(
                0,
                LlmModelOption {
                    id: configured.model.clone(),
                    label: configured.model,
                },
            );
        }
        Ok(merged)
    })
}

fn list_opencode_models() -> Result<Vec<LlmModelOption>, String> {
    let output = run_opencode(&["models", "--pure"]).or_else(|_| run_opencode(&["models"]))?;
    let output = if output.status.success() {
        output
    } else {
        run_opencode(&["models"])?
    };
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut models: Vec<LlmModelOption> =
        text.lines().filter_map(parse_opencode_model_line).collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);
    if models.is_empty() {
        Err("OpenCode returned no models".to_string())
    } else {
        Ok(models)
    }
}

fn run_opencode(args: &[&str]) -> Result<Output, String> {
    let mut command = opencode_command();
    command.args(args);
    command
        .output()
        .map_err(|e| format!("failed to start OpenCode CLI: {e}"))
}

pub fn probe_opencode_cli() -> Result<String, String> {
    let output = run_opencode(&["--version"])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() {
            format!("OpenCode returned status {}", output.status)
        } else {
            detail
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            stderr
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "OpenCode CLI available".to_string());
    Ok(detail)
}

fn opencode_command() -> Command {
    #[cfg(windows)]
    {
        if let Some((node_path, entry_path)) = opencode_windows_node_entry() {
            let mut command = Command::new(node_path);
            command.arg(entry_path);
            return command;
        }
        if let Some(path) = opencode_windows_shim() {
            if path.extension().and_then(|ext| ext.to_str()) == Some("ps1") {
                let mut command = Command::new("powershell.exe");
                command.args([
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    &path.to_string_lossy(),
                ]);
                return command;
            }
            return Command::new(path);
        }
    }
    Command::new("opencode")
}

#[cfg(windows)]
fn opencode_windows_node_entry() -> Option<(PathBuf, PathBuf)> {
    let shim = opencode_windows_shim()?;
    let base_dir = shim.parent()?.to_path_buf();
    let entry = base_dir
        .join("node_modules")
        .join("opencode-ai")
        .join("bin")
        .join("opencode");
    if !entry.exists() {
        return None;
    }
    let bundled_node = base_dir.join("node.exe");
    if bundled_node.exists() {
        Some((bundled_node, entry))
    } else {
        Some((PathBuf::from("node"), entry))
    }
}

#[cfg(windows)]
fn opencode_windows_shim() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        roots.push(PathBuf::from(appdata).join("npm"));
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        roots.push(
            PathBuf::from(userprofile)
                .join("AppData")
                .join("Roaming")
                .join("npm"),
        );
    }
    for root in roots {
        for name in ["opencode.exe", "opencode.ps1", "opencode.cmd"] {
            let candidate = root.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn parse_opencode_model_line(line: &str) -> Option<LlmModelOption> {
    let token = line
        .split_whitespace()
        .find(|part| part.contains('/') && !part.starts_with("http"))?;
    let id = token
        .trim_matches(|c: char| {
            matches!(
                c,
                ',' | '"' | '\'' | '`' | '[' | ']' | '(' | ')' | '{' | '}'
            )
        })
        .to_string();
    if id.contains('/') {
        Some(LlmModelOption {
            label: id.clone(),
            id,
        })
    } else {
        None
    }
}

fn list_ollama_models() -> Result<Vec<LlmModelOption>, String> {
    let output = Command::new("ollama")
        .arg("list")
        .output()
        .map_err(|e| format!("failed to start Ollama CLI: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut models: Vec<LlmModelOption> = text
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| !name.trim().is_empty())
        .map(|name| LlmModelOption {
            id: name.to_string(),
            label: name.to_string(),
        })
        .collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);
    if models.is_empty() {
        Err("Ollama returned no models".to_string())
    } else {
        Ok(models)
    }
}

fn fallback_models(provider: &str) -> Vec<LlmModelOption> {
    let ids = match provider {
        "opencode" => vec![
            "opencode/big-pickle",
            "opencode/minimax-m2.5-free",
            "opencode/nemotron-3-super-free",
            "opencode/ring-2.6-1t-free",
            "deepseek/deepseek-chat",
            "deepseek/deepseek-reasoner",
            "deepseek/deepseek-v4-flash",
            "deepseek/deepseek-v4-pro",
            "anthropic/claude-sonnet-4-5",
            "anthropic/claude-opus-4-1",
            "openai/gpt-5.1",
            "openai/gpt-5.1-codex",
            "google/gemini-2.5-pro",
        ],
        _ => vec!["llama3.1", "llama3.2", "mistral", "qwen2.5", "gemma2"],
    };
    ids.into_iter()
        .map(|id| LlmModelOption {
            id: id.to_string(),
            label: id.to_string(),
        })
        .collect()
}

fn provider_display_name(provider: &str) -> &'static str {
    match provider {
        "opencode" => "OpenCode",
        "ollama" | "" => "Ollama",
        _ => "LLM provider",
    }
}

fn extraction_failure_message(config: &LocalModelConfig, err: &str) -> String {
    let provider_name = provider_display_name(config.provider.as_str());
    let lower = err.to_ascii_lowercase();
    let guidance = if lower.contains("insufficient balance")
        || lower.contains("insufficient_balance")
        || lower.contains("quota")
        || lower.contains("billing")
        || lower.contains("credit")
        || lower.contains("payment required")
    {
        format!(
            "{provider_name} returned a billing/quota error. Add credits or switch provider/model in Settings and retry."
        )
    } else if lower.contains("unauthorized")
        || lower.contains("authentication")
        || lower.contains("forbidden")
        || lower.contains("api key")
        || lower.contains("login")
    {
        format!(
            "{provider_name} returned an authentication error. Re-authenticate this provider and retry."
        )
    } else {
        format!(
            "{provider_name} request failed. Check provider availability, billing, and selected model in Settings, then retry."
        )
    };
    format!("AI extraction failed. {guidance} Error: {err}")
}

#[tauri::command]
pub async fn extract_curriculum_units(
    state: DbState<'_>,
    syllabus_id: String,
) -> Result<Vec<CurriculumUnit>, String> {
    let (text, config, job_id) = state.with_conn(|conn| {
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
        conn.execute(
            "UPDATE syllabus_documents SET llm_extraction_status = 'running', updated_at = ?1 WHERE id = ?2",
            params![now(), syllabus_id],
        )
        .map_err(|e| e.to_string())?;

        let config = load_llm_config(conn);
        let job_id = create_job(
            conn,
            "syllabus_extraction",
            Some("syllabus"),
            Some(&syllabus_id),
            1,
            Some("Queued curriculum extraction"),
        )?;
        mark_job_running(conn, &job_id, Some("Extracting curriculum units"))?;
        Ok::<_, String>((text, config, job_id))
    })?;

    let units = match extract_units_with_provider(&config, &text, &syllabus_id) {
        Ok(units) => units,
        Err(err) => {
            let message = extraction_failure_message(&config, &err);
            state.with_conn(|conn| {
                conn.execute(
                    "UPDATE syllabus_documents SET llm_extraction_status = 'failed', updated_at = ?1 WHERE id = ?2",
                    params![now(), syllabus_id],
                )
                .map_err(|e| e.to_string())?;
                fail_job(conn, &job_id, &message)
            })?;
            return Err(message);
        }
    };

    state.with_conn(|conn| {
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
        conn.execute(
            "UPDATE syllabus_documents SET llm_extraction_status = 'completed', updated_at = ?1 WHERE id = ?2",
            params![now(), syllabus_id],
        )
        .map_err(|e| e.to_string())?;
        complete_job(conn, &job_id, Some("Curriculum extraction finished"))?;
        Ok(units)
    })
}

#[tauri::command]
pub async fn get_syllabus_raw_text(
    state: DbState<'_>,
    syllabus_id: String,
) -> Result<String, String> {
    state.with_conn(|conn| {
        conn.query_row(
            "SELECT COALESCE(coverage_notes, '') FROM syllabus_documents WHERE id = ?1",
            params![syllabus_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub async fn get_syllabus_extraction_diagnostics(
    state: DbState<'_>,
    syllabus_id: String,
) -> Result<SyllabusExtractionDiagnostics, String> {
    state.with_conn(|conn| {
        let text: String = conn
            .query_row(
                "SELECT COALESCE(coverage_notes, '') FROM syllabus_documents WHERE id = ?1",
                params![syllabus_id.clone()],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if text.trim().is_empty() {
            return Err("no extracted syllabus text is available".to_string());
        }
        let units = load_units(conn, &syllabus_id)?;
        let unit_count = units.len() as i32;
        let semester_layout = detect_semester_timeline_layout(&text);
        let heading_like_re = Regex::new(r"(?i)^\s*unit\s+\d{1,2}\s*[:.\-]?").expect("valid heading regex");
        let blocked_title = Regex::new(
            r"(?i)^(at the end of the unit|students will be able to|learning objective|success criteria)",
        )
        .expect("valid blocked-title regex");
        let continuation_re =
            Regex::new(r"(?i)\b(continuation|continued)\b").expect("valid continuation regex");

        let heading_like_count = units
            .iter()
            .filter(|u| heading_like_re.is_match(u.title.trim()))
            .count() as i32;
        let low_signal_count = units
            .iter()
            .filter(|u| blocked_title.is_match(u.title.trim()))
            .count() as i32;
        let continuation_count = units
            .iter()
            .filter(|u| {
                continuation_re.is_match(u.title.trim())
                    || u
                        .description
                        .as_deref()
                        .map(|d| continuation_re.is_match(d))
                        .unwrap_or(false)
            })
            .count() as i32;

        let detected_layout = if semester_layout {
            "semester_timeline".to_string()
        } else if heading_like_count >= (unit_count / 2).max(1) {
            "unit_heading_list".to_string()
        } else {
            "mixed_or_unstructured".to_string()
        };

        let mut confidence = 62i32;
        if unit_count >= 4 && unit_count <= 16 {
            confidence += 10;
        }
        if semester_layout {
            confidence += 10;
        }
        if heading_like_count >= (unit_count / 2).max(1) {
            confidence += 8;
        }
        confidence -= (low_signal_count * 10).min(30);
        confidence -= (continuation_count * 4).min(12);
        confidence = confidence.clamp(20, 98);

        let mut notes = Vec::new();
        notes.push(format!("Detected layout: {}", detected_layout.replace('_', " ")));
        notes.push(format!("Extracted {unit_count} units."));
        if semester_layout {
            notes.push("Semester/month timeline pattern was recognized and prioritized.".to_string());
        }
        if continuation_count > 0 {
            notes.push(format!(
                "Found {continuation_count} continuation-style entries; review merged pacing details."
            ));
        }
        if low_signal_count > 0 {
            notes.push(format!(
                "{low_signal_count} unit title(s) look like outcomes/objectives instead of unit names."
            ));
        }

        let mut suggested_actions = Vec::new();
        if low_signal_count > 0 {
            suggested_actions.push("Rename outcome-style unit titles to the actual module names.".to_string());
        }
        if unit_count < 4 {
            suggested_actions.push("Extraction may be under-segmented. Re-upload a clearer syllabus or edit units manually.".to_string());
        }
        if suggested_actions.is_empty() {
            suggested_actions.push("Spot-check unit order and timeline labels, then generate the lesson schedule.".to_string());
        }

        Ok(SyllabusExtractionDiagnostics {
            confidence,
            detected_layout,
            unit_count,
            notes,
            suggested_actions,
        })
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn extracts_numbered_lines() {
        let text = "1. Fractions and decimals\nLesson content about fractions with enough words to pass the minimum threshold for body content.\n\n2. Algebra basics\nIntroduction to variables and expressions with enough words to also pass the body content filter here.";
        let units = extract_units_advanced(text, "syllabus-2");
        assert!(units.len() >= 2);
    }

    #[test]
    fn extracts_semester_timeline_units() {
        let text = "Semester 1\nAugust\nUnit 1: Personal Narrative\nSeptember\nUnit 5: Description of a Process\nOctober-November\nUnit 2: News Article\nDecember\nUnit 6: Persuasive Essay\nSemester 2\nJanuary\nUnit 6: Persuasive Essay (continuation)\nFebruary\nUnit 3: Short Story\nMarch\nUnit 3: Short Story (continuation)\nApril-May\nUnit 4: Problem-and-Solution Essay";
        let units = extract_units_advanced(text, "syllabus-semester-layout");
        assert_eq!(units.len(), 6);
        assert_eq!(units[0].title, "Unit 1: Personal Narrative");
        assert_eq!(units[5].title, "Unit 4: Problem-and-Solution Essay");
        let persuasive = units
            .iter()
            .find(|u| u.title == "Unit 6: Persuasive Essay")
            .expect("unit 6 exists");
        let desc = persuasive.description.as_deref().unwrap_or("");
        assert!(desc.contains("December"));
        assert!(desc.contains("January"));
    }

    #[test]
    fn prunes_write_variants_and_unsupported_units_for_timeline_layout() {
        let text = "Semester 1\nAugust\nUnit 1: Personal Narrative\nSeptember\nUnit 5: Description of a Process\nOctober-November\nUnit 2: News Article\nDecember\nUnit 6: Persuasive Essay\nSemester 2\nJanuary\nUnit 6: Persuasive Essay (continuation)\nFebruary\nUnit 3: Short Story\nMarch\nUnit 3: Short Story (continuation)\nApril-May\nUnit 4: Problem-and-Solution Essay";
        let now_ts = now();
        let units = vec![
            (
                "Unit 6: Persuasive Essay",
                "Assigned timeline: January, December.",
            ),
            ("Unit 1: Personal Narrative", "Assigned timeline: August."),
            (
                "Unit 5: Description of a Process",
                "Assigned timeline: September.",
            ),
            (
                "Unit 2: News Article",
                "Assigned timeline: October-November.",
            ),
            ("Unit 3: Short Story", "Assigned timeline: February, March."),
            (
                "Unit 4: Problem-and-Solution Essay",
                "Assigned timeline: April-May.",
            ),
            (
                "Unit 1: Write a Personal Narrative",
                "Assigned timeline: April-May.",
            ),
            (
                "Unit 2: Write a News Article",
                "Assigned timeline: April-May.",
            ),
            (
                "Unit 3: Write a Short Story",
                "Assigned timeline: April-May.",
            ),
            (
                "Unit 4: Write a Problem-Solution Essay",
                "Assigned timeline: April-May.",
            ),
            (
                "Unit 5: Write a Description of a Process",
                "Assigned timeline: April-May.",
            ),
            (
                "Unit 6: Write a Persuasive Essay",
                "Assigned timeline: April-May.",
            ),
            ("Unit 7: Literary Analysis", "Assigned timeline: April-May."),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (title, desc))| CurriculumUnit {
            id: format!("u-{index}"),
            syllabus_id: "syllabus-qa".to_string(),
            unit_code: Some(format!("UNIT-{:03}", index + 1)),
            title: title.to_string(),
            description: Some(desc.to_string()),
            recommended_sequence: (index + 1) as i32,
            estimated_lessons: 4,
            estimated_weeks: 4,
            assessment_hint: Some("Teacher-reviewed formative assessment".to_string()),
            is_required: true,
            month_label: None,
            schedule_slot_count: 0,
            created_at: now_ts.clone(),
            updated_at: now_ts.clone(),
        })
        .collect::<Vec<_>>();

        let pruned = prune_derivative_and_unsupported_units(units, text, true);
        assert_eq!(pruned.len(), 6);
        assert!(pruned
            .iter()
            .any(|u| u.title == "Unit 1: Personal Narrative"));
        assert!(pruned.iter().any(|u| u.title == "Unit 6: Persuasive Essay"));
        assert!(!pruned.iter().any(|u| u.title.contains("Write a")));
        assert!(!pruned
            .iter()
            .any(|u| u.title == "Unit 7: Literary Analysis"));
    }

    #[test]
    fn clean_title_removes_trailing_chars() {
        let result = clean_title("Unit 3: Geometry -");
        assert!(!result.ends_with('-'));
        assert!(!result.ends_with(':'));
    }

    #[test]
    fn estimate_lessons_defaults_to_four() {
        let lessons = estimate_lessons("Unit 1", "Some content without explicit counts.");
        assert_eq!(lessons, 4);
    }

    #[test]
    fn estimate_lessons_explicit() {
        let body = "This unit requires 8 lessons total for full coverage.";
        let lessons = estimate_lessons("Unit 1", body);
        assert_eq!(lessons, 8);
    }

    #[test]
    fn estimate_lessons_from_title() {
        let body = "Basic content here.";
        let lessons = estimate_lessons("Unit 1 - SEPTEMBER CONTENT - 4 WEEKS", body);
        assert_eq!(lessons, 4);
    }

    #[test]
    fn estimate_lessons_takes_max_of_multiple_matches() {
        let body = "1 week review, 4 weeks main content.";
        let lessons = estimate_lessons("Unit 1", body);
        assert_eq!(lessons, 4);
    }

    #[test]
    fn estimate_lessons_handles_abbreviated_wks() {
        let body = "This unit covers 6 wks of instruction.";
        let lessons = estimate_lessons("Unit 1", body);
        assert_eq!(lessons, 6);
    }

    #[test]
    fn estimate_lessons_handles_en_dash_in_title() {
        let body = "Main topics and skills covered.";
        let lessons = estimate_lessons("Unit 5 â€“ Fear This â€“ 4 weeks", body);
        assert_eq!(lessons, 4);
    }

    #[test]
    fn estimate_lessons_with_month_name() {
        let lessons = estimate_lessons(
            "Unit 3 â€“ The Hero Within (February)",
            "Content covering hero archetypes.",
        );
        assert_eq!(lessons, 4);
    }

    #[test]
    fn infer_assessment_from_text() {
        assert!(infer_assessment_hint("final exam")
            .unwrap()
            .contains("test"));
        assert!(infer_assessment_hint("group project")
            .unwrap()
            .contains("Project"));
        assert!(infer_assessment_hint("weekly quiz")
            .unwrap()
            .contains("quiz"));
    }

    #[test]
    fn extracts_text_from_opencode_jsonl_events() {
        let raw = concat!(
            "{\"type\":\"step_start\",\"part\":{\"type\":\"step-start\"}}\n",
            "{\"type\":\"text\",\"part\":{\"text\":\"{\\\"v\\\":\\\"1\\\",\"}}\n",
            "{\"type\":\"text\",\"part\":{\"text\":\"\\\"objective\\\":\\\"Focus\\\"}\"}}\n"
        );
        let text = extract_text_from_jsonl_stream(raw).expect("text should be extracted");
        assert!(text.contains("\"v\":\"1\""));
        assert!(text.contains("\"objective\":\"Focus\""));
    }
    use super::*;

    #[test]
    fn extracts_heading_units() {
        let text = "Unit 1: Number Sense\nFractions decimals percentages quiz.\n\nUnit 2: Algebra\nExpressions equations project.";
        let units = extract_units_advanced(text, "syllabus-1");
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].recommended_sequence, 1);
        assert!(units[1]
            .assessment_hint
            .as_deref()
            .unwrap()
            .contains("Project"));
    }

    #[test]
    fn extracts_headings_with_en_dash() {
        let text = "Unit 5 â€“ Fear This â€“ 4 weeks\nReading comprehension, media analysis, advertising techniques.\n\nUnit 6 â€“ Are You Buying It?\nPersuasive writing, rhetorical devices.";
        let units = extract_units_advanced(text, "syllabus-en-dash");
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].estimated_lessons, 4);
    }
    #[test]
    fn extracts_heading_only_unit_lists() {
        let text = "Unit 5 - Fear This\nUnit 1 - Choices\nUnit 4 - Opening Doors (October)\nUnit 4 - Opening Doors (November)\nUnit 6 - Are You Buying It?\nUnit 3 - The Hero Within (February)\nUnit 3 - The Hero Within (March)\nUnit 2 - The Art of Expression (April)\nUnit 2 - The Art of Expression (May)";
        let units = extract_units_advanced(text, "syllabus-heading-only");
        assert_eq!(units.len(), 9);
        assert_eq!(units[0].title, "Unit 5 - Fear This");
        assert_eq!(units[8].title, "Unit 2 - The Art of Expression (May)");
    }

    #[test]
    fn falls_back_to_heuristic_when_model_returns_one_unit() {
        let text = "Unit 5 - Fear This\nUnit 1 - Choices\nUnit 4 - Opening Doors (October)\nUnit 4 - Opening Doors (November)\nUnit 6 - Are You Buying It?\nUnit 3 - The Hero Within (February)\nUnit 3 - The Hero Within (March)\nUnit 2 - The Art of Expression (April)\nUnit 2 - The Art of Expression (May)";
        let model_units = vec![ModelUnit {
            title: "Unit 1 - Fear This".to_string(),
            description: Some("Only one inferred section".to_string()),
            estimated_lessons: Some(4),
            assessment_hint: Some("quiz".to_string()),
            month_label: None,
        }];
        let units = build_curriculum_units(model_units, text, "syllabus-fallback");
        assert_eq!(units.len(), 9);
        assert_eq!(units[0].title, "Unit 5 - Fear This");
        assert_eq!(units[8].title, "Unit 2 - The Art of Expression (May)");
    }

    #[test]
    fn prefers_heuristic_when_model_collapses_heading_list() {
        let text = "Unit 1 - Choices\nUnit 4 - Opening Doors (October)\nUnit 4 - Opening Doors (November)\nUnit 6 - Are You Buying It?\nUnit 3 - The Hero Within (February)\nUnit 3 - The Hero Within (March)\nUnit 2 - The Art of Expression (April)\nUnit 2 - The Art of Expression (May)";
        let model_units = vec![
            ModelUnit {
                title: "Unit 1 Choices + Opening Doors".to_string(),
                description: Some("Combined block".to_string()),
                estimated_lessons: Some(8),
                assessment_hint: Some("quiz".to_string()),
                month_label: None,
            },
            ModelUnit {
                title: "Unit 6 Are You Buying It".to_string(),
                description: Some("Unit focus".to_string()),
                estimated_lessons: Some(3),
                assessment_hint: Some("project".to_string()),
                month_label: None,
            },
            ModelUnit {
                title: "Unit 3 The Hero Within".to_string(),
                description: Some("Combined Feb/Mar".to_string()),
                estimated_lessons: Some(8),
                assessment_hint: Some("test".to_string()),
                month_label: None,
            },
            ModelUnit {
                title: "Unit 2 The Art of Expression".to_string(),
                description: Some("Combined Apr/May".to_string()),
                estimated_lessons: Some(6),
                assessment_hint: Some("presentation".to_string()),
                month_label: None,
            },
        ];
        let units = build_curriculum_units(model_units, text, "syllabus-granularity");
        assert_eq!(units.len(), 8);
        assert_eq!(units[0].title, "Unit 1 - Choices");
        assert_eq!(units[7].title, "Unit 2 - The Art of Expression (May)");
    }

    #[test]
    fn parse_model_units_handles_control_characters() {
        let raw = "{\n  \"units\": [{\"title\": \"Unit A\", \"description\": \"bad \u{0000} text\", \"estimated_lessons\": 4, \"assessment_hint\": \"quiz\"}]\n}";
        let units = parse_model_units(raw).expect("expected sanitized parse");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].title, "Unit A");
    }

    #[test]
    fn parse_model_units_accepts_direct_array() {
        let raw = r#"[{"title":"Unit A","description":"Intro","estimated_lessons":4,"assessment_hint":"quiz"}]"#;
        let units = parse_model_units(raw).expect("expected direct-array parse");
        assert_eq!(units.len(), 1);
    }

    #[test]
    fn parse_model_units_handles_fenced_json_with_noise() {
        let raw = r#"I found these units:
```json
{
  "units": [
    {"title":"Unit A","description":"Intro","estimated_lessons":4,"assessment_hint":"quiz"}
  ]
}
```
extra footer text"#;
        let units = parse_model_units(raw).expect("expected fenced-json parse");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].title, "Unit A");
    }

    #[test]
    fn parse_model_units_handles_trailing_commas() {
        let raw = r#"{
  "units": [
    {"title":"Unit A","description":"Intro","estimated_lessons":4,"assessment_hint":"quiz",},
  ],
}"#;
        let units = parse_model_units(raw).expect("expected trailing-comma repair");
        assert_eq!(units.len(), 1);
    }
}
