# 03 — Component design, modules, and interfaces

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

This document defines the modular design needed to support:
- school → level → class → student hierarchy
- yearly calendar planning
- syllabus upload and review
- automated year-plan generation
- existing attendance/session/test/report workflows

---

## Module map

```text
frontend
  ├─ app-shell
  ├─ schools
  ├─ levels
  ├─ calendars
  ├─ syllabus
  ├─ planning
  ├─ classes
  ├─ students
  ├─ sessions
  ├─ assessments
  ├─ reports
  ├─ agents
  ├─ settings
  └─ shared-ui

backend
  ├─ command layer
  ├─ hierarchy services
  ├─ planning services
  ├─ calendar engine
  ├─ syllabus ingestion
  ├─ year plan generator
  ├─ session services
  ├─ report runtime
  ├─ agent runtime
  ├─ repositories
  ├─ file store
  └─ diagnostics
```

---

## Frontend module design

### 1) `app-shell`
Responsibilities:
- sidebar
- current school / level / class context chips
- top bar date, quick navigation, status banners
- common modals and route layout

### 2) `schools`
Responsibilities:
- school creation
- school edit
- school-level preferences
- school-level calendar defaults
- navigation into levels

Primary interfaces:
- `SchoolListView`
- `SchoolFormModal`
- `SchoolSettingsPanel`

### 3) `levels`
Responsibilities:
- level list and create/edit/archive flows
- region selection
- academic year window
- level planning readiness status
- syllabus status summary

Primary interfaces:
- `LevelListView`
- `LevelFormModal`
- `LevelOverviewPanel`
- `PlanningReadinessCard`

### 4) `calendars`
Responsibilities:
- yearly calendar creation and preview
- holiday markers
- manual closure day management
- teachable-day summary
- calendar publish/assign flows

Primary interfaces:
- `YearCalendarView`
- `CalendarEditorPanel`
- `HolidayLegend`
- `TeachableDaysSummaryCard`

### 5) `syllabus`
Responsibilities:
- upload syllabus file
- show parse/extraction status
- extracted unit review and edit
- active syllabus version selection

Primary interfaces:
- `SyllabusUploadDropzone`
- `SyllabusVersionList`
- `ExtractionReviewTable`
- `CurriculumUnitEditor`

### 6) `planning`
Responsibilities:
- class schedule rule setup
- year-plan generation wizard
- generated lesson list/calendar
- publish/regenerate controls
- risk warnings and capacity banners

Primary interfaces:
- `ScheduleRuleEditor`
- `YearPlanWizard`
- `YearPlanCalendar`
- `YearPlanLessonTable`
- `PlanningRiskBanner`

### 7) `classes`
Responsibilities:
- class list and create/edit/archive flows
- class settings
- class dashboard context
- class-specific year-plan override link
- class agent configuration entry point

Primary interfaces:
- `ClassListView`
- `ClassFormModal`
- `ClassSettingsPanel`
- `ClassHeaderSummary`

### 8) `students`
Responsibilities:
- student roster
- student profile
- student search/filter
- status summaries

Primary interfaces:
- `StudentRosterTable`
- `StudentProfileView`
- `StudentCharacterCard`

### 9) `sessions`
Responsibilities:
- lesson/session creation
- attendance capture
- actual-vs-planned lesson view
- daily dashboard integration

Primary interfaces:
- `TodayLessonCard`
- `SessionEditor`
- `AttendanceGrid`

### 10) `assessments`
Responsibilities:
- test creation
- per-student scoring
- file attachments
- optional linkage to a planned lesson

Primary interfaces:
- `AssessmentListView`
- `AssessmentDetailView`
- `ScoreEntryTable`

### 11) `reports`
Responsibilities:
- report generation setup
- report draft editing
- export actions

Primary interfaces:
- `ReportGenerationPanel`
- `ReportEditor`

### 12) `agents`
Responsibilities:
- class-level agent configuration
- per-student insight history
- future planning-agent feed

Primary interfaces:
- `AgentConfigGrid`
- `InsightFeed`

### 13) `settings`
Responsibilities:
- device/runtime settings
- model and parser settings
- feature flags
- diagnostics

### 14) `shared-ui`
Reusable components:
- chips, pills, panels, empty states
- date pickers
- weekday toggles
- file upload cards
- progress banners
- table/pagination controls

---

## Backend module design

### 1) Command layer
Receives validated IPC commands and routes to services.

### 2) Hierarchy services
Own:
- school service
- level service
- class service
- student service

### 3) Planning services
Own:
- syllabus service
- curriculum unit service
- calendar service
- schedule rule service
- year plan service

### 4) Calendar engine
Pure logic:
- holiday resolution
- academic range expansion
- teachable-day calculation
- buffer-day reservation

### 5) Syllabus ingestion
Owns:
- file-to-text extraction
- parse state tracking
- fallback parsing strategies
- structured extraction prompts

### 6) Year plan generator
Owns:
- capacity calculation
- curriculum sequencing
- LLM prompt construction
- response validation
- lesson slot materialization

### 7) Session services
Own:
- sessions
- attendance
- linkage to planned lessons

### 8) Report runtime
Owns:
- evidence bundle assembly
- prompts
- draft persistence
- export

### 9) Agent runtime
Owns:
- event subscriptions
- evaluation execution
- insight persistence

### 10) Repositories
One repository per aggregate or stable bounded context:
- `SchoolRepository`
- `LevelRepository`
- `AcademicCalendarRepository`
- `SyllabusRepository`
- `CurriculumUnitRepository`
- `YearPlanRepository`
- `ClassRepository`
- `StudentRepository`

### 11) File store
Stores:
- syllabus source files
- assessment attachments
- exported reports

### 12) Diagnostics
Stores:
- structured errors
- generation traces
- parse/generation timing
- non-PII analytics events

---

## Interface contracts between modules

### UI → command layer
Examples:
- `create_level`
- `upload_syllabus`
- `review_syllabus_extraction`
- `set_class_schedule_rules`
- `generate_year_plan`
- `publish_year_plan`

### Command layer → planning services
The command layer passes only normalized identifiers and validated payloads.

### Planning services → calendar engine
The planning service provides:
- academic range
- region holidays
- school closures
- class schedule rules
- buffer strategy

The calendar engine returns teachable-day slots and capacity summaries.

### Planning services → LLM runtime
The planning service sends:
- curriculum units
- coverage estimates
- teachable-day capacity
- pacing preferences

The LLM runtime returns a structured plan proposal that must pass schema validation before persistence.

---

## State ownership rules

- UI state owns selection and expansion only.
- backend owns academic truth.
- year plans are immutable once published; regeneration creates a new plan version.
- syllabus extraction drafts are editable review artifacts before publishing.

---

## Key module additions vs previous version

New mandatory modules:
- `levels`
- `calendars`
- `syllabus`
- `planning`
- `calendar engine`
- `year plan generator`

Existing modules with expanded responsibilities:
- `classes` now depend on `level_id`
- `sessions` can link to `year_plan_lesson_id`
- `reports` may reference year-plan context
- `agents` can consume planning-related events
