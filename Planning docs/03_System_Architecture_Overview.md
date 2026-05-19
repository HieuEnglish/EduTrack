# 03 — System architecture overview

**Project:** EduTrack  
**Version:** 1.1

---

## Architecture principle

EduTrack remains a local-first desktop application, but the architecture is now extended to support:
1. a deeper academic hierarchy: school → level → class → student
2. a built-in yearly calendar engine
3. syllabus upload, parsing, and structured extraction at level scope
4. AI-assisted year planning that converts syllabus requirements into a realistic pacing plan

---

## High-level architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│ UI Layer                                                           │
│  - school / level / class management                               │
│  - yearly calendar views                                            │
│  - syllabus upload + review                                         │
│  - year plan editor + class schedule setup                          │
│  - daily teaching dashboard                                         │
└─────────────────────────────────────────────────────────────────────┘
                ↓ IPC / command boundary
┌─────────────────────────────────────────────────────────────────────┐
│ App Core                                                            │
│  - hierarchy services                                                │
│  - planning services                                                 │
│  - calendar engine                                                   │
│  - syllabus parser orchestrator                                      │
│  - session/attendance/assessment services                            │
│  - report generation services                                        │
│  - agent runtime coordinator                                         │
└─────────────────────────────────────────────────────────────────────┘
                ↓ repositories
┌─────────────────────────────────────────────────────────────────────┐
│ Data Layer                                                          │
│  - SQLite database                                                   │
│  - file storage for syllabus/evidence/report exports                 │
│  - configuration + feature flags                                     │
│  - audit and diagnostics                                             │
└─────────────────────────────────────────────────────────────────────┘
                ↓ local process boundary
┌─────────────────────────────────────────────────────────────────────┐
│ LLM / Parser Runtime                                                │
│  - syllabus extraction                                               │
│  - year-plan generation                                              │
│  - student report generation                                         │
│  - teacher advice                                                    │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Layer definitions

### UI layer
Responsibilities:
- school, level, class, and student CRUD
- calendar creation and calendar browsing
- syllabus upload and extraction review
- schedule rule setup for each class
- year plan preview, review, publish, and regenerate flows
- daily delivery flows such as attendance, sessions, tests, and reports

The UI holds transient view state only, not business truth.

### App core
Responsibilities:
- validate commands
- enforce hierarchy rules
- calculate teachable days
- merge region holidays with school/manual closures
- build year plan inputs from syllabus + level + class schedule
- call local parser/LLM services
- persist stable output snapshots
- trigger agent refresh after operational changes

### Data layer
Responsibilities:
- store authoritative academic hierarchy
- store reusable calendars and syllabus versions
- store year plans and generated lesson slots
- store student operational history
- preserve auditability across regenerations

### LLM engine
Responsibilities:
- extract curriculum units from uploaded syllabus documents
- infer lesson/coverage estimates when source data is incomplete
- generate the yearly pacing plan
- generate student reports and advice

LLM output is advisory and reviewable. The app core never treats raw model output as automatically final without validation.

---

## Planning subsystem architecture

A new planning subsystem sits beside the operational classroom subsystem.

### Inputs
- level metadata
- selected region
- academic year dates
- school/manual closure dates
- class weekly schedule rules
- active syllabus document
- extracted curriculum units
- planning configuration (buffer %, pacing mode, fallback lesson estimates)

### Outputs
- normalized teachable-day calendar
- published year plan
- generated lesson slots
- risk flags such as under-capacity or impossible completion
- class-specific overrides when needed

### Services
- `CalendarService`
- `HolidayResolutionService`
- `SyllabusIngestionService`
- `CurriculumExtractionService`
- `YearPlanGenerationService`
- `YearPlanReviewService`

---

## Data flow — syllabus upload to published year plan

```text
Teacher selects School → Level
  → Uploads syllabus file
  → UI dispatches: upload_syllabus { level_id, attachment }
  → App Core stores attachment metadata + file
  → App Core creates syllabus row with parser_status=pending
  → Parser orchestrator reads file text
  → LLM extraction builds curriculum units / outcomes / estimated coverage
  → App Core stores extraction result as reviewable draft
  → Teacher reviews extracted units
  → Teacher sets region + academic year + calendar defaults
  → Teacher defines class meeting days
  → UI dispatches: generate_year_plan { level_id, class_id? }
  → App Core counts teachable days from selected calendar
  → App Core excludes holidays/closure days
  → App Core sends structured curriculum + teachable-day capacity to local LLM
  → LLM returns lesson sequencing proposal
  → App Core validates date alignment and capacity
  → App Core stores YearPlan + YearPlanLesson rows
  → Teacher reviews and publishes
```

---

## Data flow — daily session (write path)

```text
Teacher opens class dashboard
  → Selected class already linked to level and plan
  → UI shows today's planned lesson if one exists
  → Teacher marks attendance
      { session_id, student_id, status }
  → App Core validates class/student/session linkage
  → AttendanceRecord upsert
  → Session can be linked to year_plan_lesson_id
  → On completion, YearPlanLesson status may update to completed
  → Agent refresh runs for affected student/class
```

---

## Data flow — report generation

```text
Teacher opens student report generation
  → App Core fetches:
      - attendance history
      - assessments and files
      - session notes
      - agent insights
      - optional year-plan / syllabus coverage context
  → App Core builds report prompt
  → Local LLM generates school report + teacher advice
  → App Core stores report draft
  → UI shows editable report editor
```

---

## Layered validation strategy

### 1) Command validation
Examples:
- class must belong to selected level
- syllabus must belong to selected level
- year plan cannot generate without active meeting days
- academic year end must be after start

### 2) Domain validation
Examples:
- no year-plan lesson on excluded holiday date
- no duplicate weekday schedule rule in MVP
- active student cap stays at 50
- only one active published year plan per scope

### 3) Persistence validation
Examples:
- foreign keys enforce hierarchy
- uniqueness prevents duplicate schedule rules and calendar defaults
- superseded plans remain historically readable

---

## Agent architecture

Each agent is still a local module implementing a shared trait, but the event system is extended with planning-aware events.

Examples of new event topics:
- `syllabus_uploaded`
- `syllabus_reviewed`
- `year_plan_generated`
- `year_plan_published`
- `lesson_completed_against_plan`
- `coverage_drift_detected`

This allows future planning agents without rewriting the core event bus.

---

## Security architecture

Planning introduces new sensitive assets:
- syllabus source files
- year plans
- inferred curriculum structures
- region/calendar configuration

Controls:
- local-only by default
- encrypted local storage where configured
- no silent external holiday or parser upload of syllabus contents
- visible status if any external provider is ever added later

---

## Optional cloud sync (post-MVP architecture note)

If cloud sync is introduced later, the system should sync:
- hierarchy metadata
- calendars
- published plans
- operational records

It should not sync raw syllabus files or report contents by default without explicit school approval.
