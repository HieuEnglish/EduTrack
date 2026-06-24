-- EduTrack Database Schema
-- Based on 04_Database_Schema_And_Migrations.md

PRAGMA foreign_keys = ON;

-- Schools table
CREATE TABLE schools (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  logo_path TEXT,
  country_code TEXT,
  region_code TEXT,
  timezone TEXT NOT NULL DEFAULT 'UTC',
  academic_year_label TEXT,
  default_report_word_count INTEGER NOT NULL DEFAULT 200,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

-- Levels table
CREATE TABLE levels (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  code TEXT,
  display_order INTEGER NOT NULL DEFAULT 0,
  academic_year_start TEXT NOT NULL,
  academic_year_end TEXT NOT NULL,
  region_code TEXT NOT NULL,
  timezone TEXT NOT NULL,
  default_syllabus_id TEXT,
  default_calendar_id TEXT,
  planning_status TEXT NOT NULL DEFAULT 'draft',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX idx_levels_school_id ON levels(school_id);
CREATE UNIQUE INDEX idx_levels_school_order ON levels(school_id, display_order) WHERE archived_at IS NULL;

-- Academic calendars table
CREATE TABLE academic_calendars (
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

CREATE INDEX idx_academic_calendars_school_level ON academic_calendars(school_id, level_id);

-- Calendar closure days table
CREATE TABLE calendar_closure_days (
  id TEXT PRIMARY KEY,
  calendar_id TEXT NOT NULL REFERENCES academic_calendars(id) ON DELETE CASCADE,
  closure_date TEXT NOT NULL,
  closure_type TEXT NOT NULL, -- holiday | manual
  title TEXT,
  source_provider TEXT,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_calendar_closure_unique
ON calendar_closure_days(calendar_id, closure_date, closure_type);

-- Syllabus documents table
CREATE TABLE syllabus_documents (
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

CREATE INDEX idx_syllabus_documents_level_id ON syllabus_documents(level_id);
CREATE UNIQUE INDEX idx_syllabus_documents_one_active
ON syllabus_documents(level_id)
WHERE is_active = 1 AND archived_at IS NULL;

-- Curriculum units table
CREATE TABLE curriculum_units (
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

CREATE INDEX idx_curriculum_units_syllabus_id ON curriculum_units(syllabus_id);
CREATE UNIQUE INDEX idx_curriculum_units_sequence ON curriculum_units(syllabus_id, recommended_sequence);

-- Classes table
CREATE TABLE classes (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
  level_id TEXT NOT NULL REFERENCES levels(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  subject_name TEXT,
  academic_year_label TEXT,
  start_date TEXT,
  end_date TEXT,
  period_minutes INTEGER NOT NULL DEFAULT 45,
  max_students INTEGER NOT NULL DEFAULT 50,
  schedule_pattern_summary TEXT,
  year_plan_id TEXT,
  teacher_notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX idx_classes_level_id ON classes(level_id);
CREATE UNIQUE INDEX idx_classes_level_name_active ON classes(level_id, name) WHERE archived_at IS NULL;

-- Class schedule rules table
CREATE TABLE class_schedule_rules (
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

CREATE UNIQUE INDEX idx_class_schedule_unique
ON class_schedule_rules(class_id, weekday, COALESCE(period_label, ''))
WHERE is_active = 1;

-- Students table
CREATE TABLE students (
  id TEXT PRIMARY KEY,
  class_id TEXT NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
  school_id TEXT NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
  level_id TEXT NOT NULL REFERENCES levels(id) ON DELETE CASCADE,
  full_name TEXT NOT NULL,
  preferred_name TEXT,
  student_code TEXT,
  age INTEGER,
  gender TEXT,
  avatar_seed TEXT,
  notes_private TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX idx_students_class_id ON students(class_id);

-- Student characters table
CREATE TABLE student_characters (
  id TEXT PRIMARY KEY,
  student_id TEXT NOT NULL UNIQUE REFERENCES students(id) ON DELETE CASCADE,
  nickname TEXT NOT NULL,
  archetype_key TEXT NOT NULL,
  level_current INTEGER NOT NULL DEFAULT 1,
  level_max INTEGER NOT NULL DEFAULT 10,
  xp_points INTEGER NOT NULL DEFAULT 0,
  last_level_up_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Year plans table
CREATE TABLE year_plans (
  id TEXT PRIMARY KEY,
  school_id TEXT NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
  level_id TEXT NOT NULL REFERENCES levels(id) ON DELETE CASCADE,
  class_id TEXT REFERENCES classes(id) ON DELETE CASCADE,
  syllabus_id TEXT NOT NULL REFERENCES syllabus_documents(id),
  calendar_id TEXT NOT NULL REFERENCES academic_calendars(id),
  region_code TEXT NOT NULL,
  plan_scope TEXT NOT NULL, -- level_default | class_specific
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

CREATE INDEX idx_year_plans_level_id ON year_plans(level_id);
CREATE INDEX idx_year_plans_class_id ON year_plans(class_id);

-- Year plan lessons table
CREATE TABLE year_plan_lessons (
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

CREATE INDEX idx_year_plan_lessons_plan_id ON year_plan_lessons(year_plan_id);
CREATE UNIQUE INDEX idx_year_plan_lessons_plan_date ON year_plan_lessons(year_plan_id, teaching_date);
CREATE UNIQUE INDEX idx_year_plan_lessons_plan_seq ON year_plan_lessons(year_plan_id, sequence_number);

-- Class agent assignments table
CREATE TABLE class_agent_assignments (
  id TEXT PRIMARY KEY,
  class_id TEXT NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
  agent_type TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  config_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Sessions table
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  class_id TEXT NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
  year_plan_lesson_id TEXT REFERENCES year_plan_lessons(id),
  session_date TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  topic_tags_json TEXT NOT NULL DEFAULT '[]',
  completed INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_sessions_class_date ON sessions(class_id, session_date);

-- Attendance records table
CREATE TABLE attendance_records (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  student_id TEXT NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  note TEXT,
  recorded_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_attendance_unique ON attendance_records(session_id, student_id);

-- Assessments table
CREATE TABLE assessments (
  id TEXT PRIMARY KEY,
  class_id TEXT NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
  year_plan_lesson_id TEXT REFERENCES year_plan_lessons(id),
  title TEXT NOT NULL,
  assessment_date TEXT NOT NULL,
  description TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Assessment scores table
CREATE TABLE assessment_scores (
  id TEXT PRIMARY KEY,
  assessment_id TEXT NOT NULL REFERENCES assessments(id) ON DELETE CASCADE,
  student_id TEXT NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  score_value REAL,
  score_label TEXT,
  teacher_comment TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_assessment_scores_unique ON assessment_scores(assessment_id, student_id);

-- Attachments table
CREATE TABLE attachments (
  id TEXT PRIMARY KEY,
  owner_type TEXT NOT NULL,
  owner_id TEXT NOT NULL,
  file_name TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  storage_path TEXT NOT NULL,
  sha256 TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_attachments_owner ON attachments(owner_type, owner_id);

-- Agent insights table
CREATE TABLE agent_insights (
  id TEXT PRIMARY KEY,
  class_id TEXT NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
  student_id TEXT REFERENCES students(id) ON DELETE CASCADE,
  agent_type TEXT NOT NULL,
  severity TEXT NOT NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  source_event_type TEXT,
  created_at TEXT NOT NULL
);

-- Student reports table
CREATE TABLE student_reports (
  id TEXT PRIMARY KEY,
  student_id TEXT NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  class_id TEXT NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
  generated_from_plan_id TEXT REFERENCES year_plans(id),
  word_count_target INTEGER NOT NULL,
  report_text TEXT NOT NULL,
  teacher_advice_text TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Feature flags table
CREATE TABLE feature_flags (
  key TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL,
  scope_id TEXT,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Settings table
CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Audit events table
CREATE TABLE audit_events (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  actor_type TEXT NOT NULL,
  payload_json TEXT,
  created_at TEXT NOT NULL
);

-- Long-running job records
CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  job_type TEXT NOT NULL,
  entity_type TEXT,
  entity_id TEXT,
  status TEXT NOT NULL,
  progress_current INTEGER NOT NULL DEFAULT 0,
  progress_total INTEGER NOT NULL DEFAULT 0,
  message TEXT,
  error TEXT,
  cancel_requested INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  started_at TEXT,
  finished_at TEXT,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_jobs_status_updated ON jobs(status, updated_at DESC);
CREATE INDEX idx_jobs_entity ON jobs(entity_type, entity_id);

-- Schema migrations table
CREATE TABLE schema_migrations (
  version TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL
);
