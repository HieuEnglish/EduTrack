# 01 — Success metrics and analytics plan

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

This document defines success metrics for the expanded product, now including level-based planning, calendars, and syllabus-driven year plans.

---

## Success framework

EduTrack succeeds if it becomes both:
1. the teacher’s daily classroom record, and
2. the teacher’s planning system for completing a level syllabus over the academic year.

### Primary outcome metrics
1. planning setup completion
2. attendance capture consistency
3. syllabus-to-plan publish rate
4. report generation adoption
5. report editing time saved
6. retention of class history over term/year

---

## North-star metric

**Percentage of active classes with both:**
- a published year plan, and
- at least one completed session logged in the current month.

This measures whether the app is being used for both planning and execution.

---

## Core product metrics

### Adoption metrics
- number of schools created
- number of levels created
- number of active classes
- number of classes with schedule rules configured
- number of levels with active syllabus uploaded
- number of levels with published year plan

### Workflow completion metrics
- percentage of new schools that create at least one level
- percentage of new levels that upload a syllabus
- percentage of classes with at least one meeting day selected
- percentage of planning-ready levels that successfully publish a year plan
- percentage of classes used weekly
- percentage of sessions with attendance completion

### Reporting metrics
- reports generated per class per period
- report generation success rate
- percentage of generated reports edited before export

### Insight and planning quality metrics
- warnings per generated plan
- percentage of plans regenerated after disruption
- percentage of classes drifting behind plan
- percentage of completed sessions linked to planned lessons

### Reliability metrics
- syllabus parse failure rate
- year-plan generation failure rate
- report failure rate
- average time from syllabus upload to published plan

---

## Quality-of-outcome metrics

- percentage of students with continuous attendance history
- percentage of levels with complete syllabus coverage by end of year
- average completeness score of report evidence bundle
- percentage of year plans completed on time
- teacher override volume on generated plans
- report regeneration rate

High regeneration or heavy override may indicate poor planning/report output quality.

---

## Privacy-preserving measurement approach

### MVP default
Analytics should default to local-only diagnostics unless the school/teacher explicitly enables sharing later.

### Event design principle
Track product behaviour, not raw educational content.

Do not log:
- raw syllabus text
- raw report text
- student names
- teacher private notes

---

## Suggested local event catalogue

- `school_created`
- `level_created`
- `class_created`
- `syllabus_uploaded`
- `syllabus_reviewed`
- `calendar_created`
- `class_schedule_saved`
- `year_plan_generated`
- `year_plan_published`
- `year_plan_regenerated`
- `session_created`
- `attendance_marked`
- `assessment_created`
- `report_generation_started`
- `report_generation_completed`

Example shape:

```json
{
  "event": "year_plan_published",
  "school_id": "uuid",
  "level_id": "uuid",
  "class_id": null,
  "calendar_id": "uuid",
  "syllabus_id": "uuid",
  "warning_count": 1,
  "buffer_days_reserved": 8,
  "ts": "2026-05-11T10:00:00Z"
}
```

---

## KPIs for first release period

- 80% of created classes have schedule rules set
- 70% of created levels upload at least one syllabus
- 60% of planning-ready levels publish a year plan
- 85% of active classes record at least one session per week
- 90% of report generations complete successfully

These are initial directional targets and should be refined after pilot usage.

---

## In-app analytics surfaces

- school summary card
- level planning readiness card
- class execution health card
- plan coverage / drift indicator
- report usage summary

---

## Data retention for analytics events

- keep local event rows only as long as needed for trend charts and debugging
- aggregate where possible
- make deletion easy when school policy requires it
