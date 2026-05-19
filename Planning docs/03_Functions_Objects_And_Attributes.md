# 03 — Functions, objects, and attributes

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

This document defines the updated EduTrack domain model after introducing:
- school → level/grade → class → student hierarchy
- yearly academic calendar planning
- syllabus upload at the level/grade layer
- AI-generated yearly lesson pacing based on syllabus, region, holidays, and class meeting days

---

## Object model overview

The core structure is now:

1. **School**
2. **Level**
3. **Class**
4. **Student**
5. **StudentCharacter**
6. **AcademicCalendar**
7. **HolidayRegion**
8. **SyllabusDocument**
9. **CurriculumUnit**
10. **ClassScheduleRule**
11. **YearPlan**
12. **YearPlanLesson**
13. **Session**
14. **AttendanceRecord**
15. **Assessment**
16. **AssessmentScore**
17. **Attachment**
18. **AgentInsight**
19. **StudentReport**

---

## 1) School

Represents a top-level teaching organisation managed in the app.

### Attributes
- `id: UUID`
- `name: string`
- `logo_path: string | null`
- `country_code: string | null`
- `region_code: string | null`
- `timezone: string`
- `academic_year_label: string | null`
- `default_report_word_count: integer`
- `created_at: datetime`
- `updated_at: datetime`
- `archived_at: datetime | null`

### Functions
- create school
- update school details
- archive school
- list schools
- set school defaults
- export school data
- configure academic region defaults

### Rules
- school name is required
- a school can contain many levels
- school region defaults may be overridden at level/class level only where explicitly allowed

---

## 2) Level

Represents a grade, year level, or stage inside a school.

### Attributes
- `id: UUID`
- `school_id: UUID`
- `name: string`
- `code: string | null`
- `display_order: integer`
- `academic_year_start: date`
- `academic_year_end: date`
- `region_code: string`
- `timezone: string`
- `default_syllabus_id: UUID | null`
- `default_calendar_id: UUID | null`
- `planning_status: enum(draft, ready, active, archived)`
- `created_at: datetime`
- `updated_at: datetime`
- `archived_at: datetime | null`

### Functions
- create level
- edit level
- archive level
- list levels by school
- upload syllabus for level
- set region and academic year window
- generate or regenerate year plan
- publish level planning defaults to classes

### Rules
- a level belongs to exactly one school
- a school may have many levels
- each active level should have one active academic year range
- a level may have multiple syllabus uploads over time, but only one active default syllabus
- year planning should only be publishable when required inputs are complete

---

## 3) Class

Represents a teaching group inside a level.

### Attributes
- `id: UUID`
- `school_id: UUID`
- `level_id: UUID`
- `name: string`
- `subject_name: string | null`
- `academic_year_label: string | null`
- `max_students: integer`
- `schedule_pattern_summary: string | null`
- `meeting_days_mask: string | null`
- `year_plan_id: UUID | null`
- `teacher_notes: text | null`
- `created_at: datetime`
- `updated_at: datetime`
- `archived_at: datetime | null`

### Functions
- create class
- edit class
- archive class
- list classes by level
- switch active class context
- assign agents to class
- define class meeting days
- attach class-specific overrides to generated year plan

### Rules
- a class belongs to exactly one level
- active student count may not exceed 50 in MVP
- a class may meet on one or many weekdays
- class meeting rules must be compatible with the active academic calendar
- a class may inherit a year plan from level defaults or use a class-specific generated plan

---

## 4) Student

Represents a learner enrolled in a class.

### Attributes
- `id: UUID`
- `class_id: UUID`
- `school_id: UUID`
- `level_id: UUID`
- `full_name: string`
- `preferred_name: string | null`
- `student_code: string | null`
- `avatar_seed: string | null`
- `notes_private: text | null`
- `created_at: datetime`
- `updated_at: datetime`
- `archived_at: datetime | null`

### Derived attributes
- `attendance_rate`
- `average_score`
- `latest_character_level`
- `report_count`
- `last_session_at`

### Functions
- add student
- update student
- move/archive student
- search students in class
- list students by class
- generate report for student

### Rules
- student belongs to one class in MVP
- archived students remain visible in history contexts
- student identifiers should be optional unless school policy requires them

---

## 5) StudentCharacter

Represents the mascot/avatar progression layer for a student.

### Attributes
- `id: UUID`
- `student_id: UUID`
- `nickname: string`
- `archetype_key: string`
- `level_current: integer`
- `level_max: integer`
- `xp_points: integer`
- `last_level_up_at: datetime | null`

### Functions
- create default character on student creation
- update nickname
- award xp
- level up automatically
- grant manual level boost

### Rules
- each student has exactly one active character profile
- level must be between 1 and configured max
- nickname should be unique within a class for teacher usability

---

## 6) AcademicCalendar

Represents the teaching calendar used for year planning.

### Attributes
- `id: UUID`
- `school_id: UUID`
- `level_id: UUID | null`
- `name: string`
- `academic_year_start: date`
- `academic_year_end: date`
- `region_code: string`
- `timezone: string`
- `is_default: boolean`
- `created_at: datetime`
- `updated_at: datetime`

### Functions
- create calendar
- duplicate calendar
- set default calendar
- add manual closure day
- remove closure day
- preview teachable days
- publish to classes

### Rules
- calendars may exist at school level or level level
- only one default active calendar per scope
- generated year plans must snapshot the calendar inputs they used

---

## 7) HolidayRegion

Represents the regional holiday source used by planning logic.

### Attributes
- `code: string`
- `country_code: string`
- `subregion_code: string | null`
- `display_name: string`
- `provider_key: string`
- `supports_school_overrides: boolean`

### Functions
- list supported regions
- sync holiday dates
- validate region availability

### Rules
- region selection is required before automated year planning
- missing or partial region data must surface clearly to the user

---

## 8) SyllabusDocument

Represents a syllabus uploaded for a level.

### Attributes
- `id: UUID`
- `school_id: UUID`
- `level_id: UUID`
- `title: string`
- `version_label: string | null`
- `source_file_attachment_id: UUID`
- `parser_status: enum(pending, parsed, failed, reviewed)`
- `llm_extraction_status: enum(not_started, running, completed, failed)`
- `coverage_notes: text | null`
- `created_at: datetime`
- `updated_at: datetime`
- `archived_at: datetime | null`

### Functions
- upload syllabus
- replace syllabus file
- parse syllabus
- approve extracted units
- archive syllabus
- set active syllabus

### Rules
- a level can have multiple historical syllabus versions
- only one syllabus may be active for automatic planning at a time
- planning cannot be published from an unreviewed failed parse

---

## 9) CurriculumUnit

Represents a structured unit/outcome extracted from a syllabus.

### Attributes
- `id: UUID`
- `syllabus_id: UUID`
- `unit_code: string | null`
- `title: string`
- `description: text | null`
- `recommended_sequence: integer`
- `estimated_lessons: integer | null`
- `estimated_weeks: integer | null`
- `assessment_hint: text | null`
- `is_required: boolean`

### Functions
- add unit
- edit extracted unit
- reorder units
- merge/split units
- mark required/optional

### Rules
- sequence must be deterministic for year plan generation
- estimated lessons should default conservatively if missing from source

---

## 10) ClassScheduleRule

Represents the weekly teaching pattern for a class.

### Attributes
- `id: UUID`
- `class_id: UUID`
- `weekday: enum(mon, tue, wed, thu, fri, sat, sun)`
- `start_time: string | null`
- `end_time: string | null`
- `period_label: string | null`
- `is_active: boolean`

### Functions
- add meeting day
- remove meeting day
- reorder display
- validate overlap
- summarise weekly pattern

### Rules
- a class must have at least one active meeting day before year planning can run
- duplicate weekday/period entries are not allowed in MVP unless multi-period support is enabled later

---

## 11) YearPlan

Represents the AI-assisted yearly pacing plan created from syllabus + calendar + schedule rules.

### Attributes
- `id: UUID`
- `school_id: UUID`
- `level_id: UUID`
- `class_id: UUID | null`
- `syllabus_id: UUID`
- `calendar_id: UUID`
- `region_code: string`
- `plan_scope: enum(level_default, class_specific)`
- `generation_status: enum(draft, generated, reviewed, published, superseded, failed)`
- `generation_summary: text | null`
- `total_teaching_days: integer`
- `total_planned_lessons: integer`
- `holiday_days_excluded: integer`
- `buffer_days_reserved: integer`
- `generated_at: datetime | null`
- `published_at: datetime | null`

### Functions
- generate year plan
- regenerate plan
- review plan
- publish plan
- clone to class
- supersede older plan

### Rules
- level default plan may be shared by multiple classes
- class-specific plan may override level plan
- generation must account for holidays and non-teaching days
- a published plan should remain reproducible from its snapshot inputs

---

## 12) YearPlanLesson

Represents one scheduled lesson slot in the yearly plan.

### Attributes
- `id: UUID`
- `year_plan_id: UUID`
- `teaching_date: date`
- `weekday: string`
- `sequence_number: integer`
- `curriculum_unit_id: UUID | null`
- `lesson_title: string`
- `lesson_objective: text | null`
- `coverage_weight: decimal | null`
- `is_buffer: boolean`
- `is_holiday_adjusted: boolean`
- `status: enum(planned, completed, skipped, moved)`

### Functions
- assign curriculum unit
- edit lesson title/objective
- move lesson
- mark completed
- mark skipped
- convert to buffer
- regenerate forward from a point

### Rules
- each generated lesson must map to a teachable day
- date collisions within the same class plan are not allowed
- buffer lessons may exist to absorb disruption without breaking syllabus completion targets

---

## 13) Session

Represents an actual delivered class session.

### Attributes
- `id: UUID`
- `class_id: UUID`
- `year_plan_lesson_id: UUID | null`
- `session_date: date`
- `title: string`
- `description: text | null`
- `topic_tags: string[]`
- `completed: boolean`
- `created_at: datetime`
- `updated_at: datetime`

### Functions
- create session
- edit session
- link to plan lesson
- mark completed

### Rules
- sessions may be linked or unlinked from the planned lesson
- historical sessions must remain even if the year plan changes later

---

## 14) AttendanceRecord

### Attributes
- `id: UUID`
- `session_id: UUID`
- `student_id: UUID`
- `status: enum(present, late, absent, excused)`
- `note: string | null`
- `recorded_at: datetime`

### Functions
- mark attendance
- bulk mark attendance
- update status
- add note

### Rules
- one active attendance record per session per student
- attendance should remain editable after save

---

## 15) Assessment

### Attributes
- `id: UUID`
- `class_id: UUID`
- `year_plan_lesson_id: UUID | null`
- `title: string`
- `assessment_date: date`
- `description: text | null`

### Functions
- create assessment
- edit assessment
- link to planned lesson
- attach files

### Rules
- assessments belong to a class
- linking to the year plan is optional but recommended

---

## 16) AssessmentScore

### Attributes
- `id: UUID`
- `assessment_id: UUID`
- `student_id: UUID`
- `score_value: decimal`
- `score_label: string | null`
- `teacher_comment: text | null`

### Functions
- record score
- edit score
- bulk import scores

### Rules
- one score row per assessment per student in MVP
- null score is allowed only before grading completes

---

## 17) Attachment

### Attributes
- `id: UUID`
- `owner_type: enum(assessment, syllabus, report, session)`
- `owner_id: UUID`
- `file_name: string`
- `mime_type: string`
- `size_bytes: integer`
- `storage_path: string`

### Functions
- attach file
- replace file
- detach file

### Rules
- syllabus upload uses the same attachment subsystem as evidence files
- attachment ownership must be explicit for deletion safety

---

## 18) AgentInsight

### Attributes
- `id: UUID`
- `class_id: UUID`
- `student_id: UUID | null`
- `agent_type: string`
- `severity: enum(info, success, warning)`
- `title: string`
- `body: text`
- `created_at: datetime`

### Functions
- evaluate class/student
- store insight
- dismiss/action

### Rules
- planning-specific agent insights may later be added without changing the hierarchy model

---

## 19) StudentReport

### Attributes
- `id: UUID`
- `student_id: UUID`
- `class_id: UUID`
- `word_count_target: integer`
- `report_text: text`
- `teacher_advice_text: text | null`
- `generated_from_plan_id: UUID | null`
- `generated_at: datetime`

### Functions
- generate report
- regenerate report
- save draft
- export report

### Rules
- reports are drafts first, never final by default
- report generation may include syllabus/year-plan context where useful for evidence framing

---

## Key derived behaviours

### A) Level planning readiness
A level is planning-ready when it has:
- active academic year start/end dates
- region selected
- default calendar available
- active syllabus uploaded and parsed/reviewed
- at least one class with meeting-day rules

### B) Smart year planning
The LLM-assisted planner should:
- read extracted curriculum units
- estimate coverage needs where source detail is incomplete
- count valid teaching days from the academic calendar
- exclude holidays and manual closure days
- reserve configurable buffer capacity
- distribute coverage across actual class meeting days
- flag impossible or high-risk completion scenarios

### C) Hierarchy inheritance
- school provides broad defaults
- level owns syllabus and yearly planning defaults
- class owns delivery schedule and optional plan overrides
- student remains tied to class execution and reporting data
