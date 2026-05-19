# 04 — Data plan

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

This document defines what data EduTrack stores, how it is grouped, and how the new planning system changes data responsibilities.

---

## Data categories

### 1) Core operational data
- schools
- levels
- classes
- students
- sessions
- attendance records
- assessments
- assessment scores

### 2) Planning data
- academic calendars
- closure days / holidays
- class meeting-day rules
- syllabus documents
- extracted curriculum units
- generated year plans
- year-plan lessons

### 3) Evidence data
- uploaded syllabus source files
- assessment attachments
- report exports

### 4) Derived data
- attendance rate
- score trends
- teachable-day counts
- completion-risk flags
- syllabus coverage progress

### 5) AI-generated data
- curriculum extraction outputs
- yearly pacing suggestions
- agent insights
- student report drafts
- teacher advice

### 6) Configuration data
- school defaults
- level defaults
- class configuration
- model settings
- parser settings
- feature flags

### 7) Diagnostics and audit data
- generation timings
- parse failures
- publish actions
- audit events

---

## Data ownership and scope

### School-scoped
- school profile
- school-wide defaults
- optional school calendar defaults

### Level-scoped
- academic year range
- region selection
- default calendar
- syllabus uploads
- year-plan defaults

### Class-scoped
- class identity
- meeting days
- class-specific plan override
- attendance/session/assessment history
- assigned agents

### Student-scoped
- student profile
- character progression
- attendance history
- scores
- reports

### Device-scoped
- local model configuration
- parser runtime configuration
- cached diagnostics

---

## Source-of-truth rules

1. SQLite is the source of truth for structured records.
2. Source files live in local file storage; DB stores metadata and ownership.
3. Published year plans are stored as versioned records plus lesson rows, not recomputed on every view.
4. Region holidays are copied into local closure-day rows when adopted into a calendar snapshot.
5. Operational delivery history remains valid even if a year plan is later regenerated.

---

## Data flow by feature

### Level planning
Input:
- level metadata
- academic year dates
- region code
- active syllabus
- class meeting-day rules

Output:
- teachable-day count
- generated lesson schedule
- warnings and risk flags

### Daily attendance
Input:
- selected class
- active session or lesson
- student status entries

Output:
- attendance records
- updated attendance summary
- optional plan progress update

### Session logging
Input:
- session date
- lesson title/notes
- optional year-plan lesson link

Output:
- session history
- lesson completion tracking

### Assessments and files
Input:
- assessment details
- scores
- evidence files

Output:
- score history
- attachment metadata
- report evidence bundle

### Reports
Input:
- student data
- class/session history
- assessment evidence
- insights
- optional year-plan context

Output:
- report draft
- private teacher advice
- export artifact

---

## Retention guidelines

### Keep until teacher/school deletes or archives
- schools, levels, classes
- students
- sessions
- assessments
- reports

### Keep while active plan history matters
- syllabus files
- curriculum units
- year plans
- plan lessons

### Safe to prune with policy or user action
- failed parse intermediates
- temporary generation traces
- stale local caches

---

## Data minimisation guidance

- one active syllabus per level is sufficient for planning
- do not store redundant holiday data outside the adopted calendar snapshot
- do not duplicate large syllabus text blobs in analytics/audit records
- avoid copying student names into diagnostics

---

## New critical integrity relationships

- every class must map to a level
- every level must map to a school
- every plan must map to syllabus + calendar + level
- class-specific plans may optionally map to class
- sessions may map to planned lessons, but not required

---

## Export considerations

Export bundles may later include:
- school metadata
- level planning package
- class operational history
- student reports

Syllabus files and student records must remain separable so schools can export planning without over-exporting student data.
