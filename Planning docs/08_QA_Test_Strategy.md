# 08 — QA test strategy

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

This document defines how EduTrack is tested after the addition of levels, calendars, syllabus upload, and year planning.

---

## Test objectives

1. The hierarchy remains consistent: school → level → class → student.
2. No student data is silently lost or corrupted.
3. Planning logic respects region holidays and class meeting days.
4. Syllabus extraction and year-plan generation fail safely.
5. Existing classroom workflows remain fast and reliable.

---

## Quality scope

- hierarchy CRUD
- planning setup
- calendar logic
- syllabus upload and extraction
- year-plan generation and publishing
- attendance flows
- assessments
- reporting
- security/privacy basics
- accessibility

---

## Test pyramid

### 1) Unit tests
- teachable-day calculations
- level/class inheritance rules
- schedule rule validation
- coverage and buffer math
- character progression rules

### 2) Integration tests
- upload syllabus → parse → review → generate year plan
- create class → add schedule days → publish plan
- complete session linked to planned lesson
- report generation with plan context

### 3) End-to-end tests
- first launch → create school → level → class → student
- upload syllabus and publish a year plan
- mark attendance from dashboard
- add a test and attach files
- generate a report and save draft

### 4) Exploratory/manual testing
- review long syllabi
- inspect plan calendar readability
- try disruption/regeneration workflow
- confirm user understanding of hierarchy

---

## Environment matrix

### Development
- fast local DB resets
- parser/LLM mock mode

### CI
- deterministic fixtures
- no real external holiday dependency

### Pre-release staging build
- realistic sample syllabi
- sample regional calendars
- 50-student class stress checks

---

## Core test suites

### A. Domain rules suite
- class max 50 students
- valid hierarchy references only
- no duplicate active meeting-day rule
- no year-plan publish without reviewed syllabus when flag enabled

### B. Persistence suite
- migration backfills default levels correctly
- generated plan snapshots persist
- published plans remain immutable
- session links to planned lessons survive reload

### C. File/attachment suite
- syllabus upload accepted for supported file types
- oversized file rejected cleanly
- attachment replacement keeps audit trail

### D. AI/planning suite
- syllabus extraction progress events emitted
- malformed extraction handled
- impossible completion warning generated
- regeneration from date preserves historical sessions
- advice/report failure does not lose generated report

### E. Security/privacy suite
- no raw report text in production logs
- no raw syllabus text in production logs
- private teacher advice excluded from export unless selected

### F. Accessibility/UI suite
- keyboard navigation for weekday picker
- calendar cells have screen-reader labels
- upload progress states announced
- contrast acceptable in planning banners

---

## Key regression scenarios

1. Existing schools/classes migrate into default levels safely.
2. Classes still work even if planning modules are disabled.
3. Attendance remains sub-second in normal usage.
4. Report generation still works when no year plan is linked.
5. Calendar changes do not delete historical session data.

---

## Performance targets

- dashboard load for 50-student class: acceptable without major jank
- attendance writes feel instant
- syllabus parse status appears promptly
- year-plan generation completes within acceptable local-runtime budget
- calendar month view remains responsive for full academic year

---

## UAT checklist

- user can create school, level, class, and student
- user can upload syllabus for a level
- user can choose class meeting days
- user can generate a plan that excludes holidays
- user can publish and later regenerate a plan
- user can still run day-to-day teaching workflows without confusion
