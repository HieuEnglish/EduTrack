# 07 — Security, privacy, and compliance plan

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

EduTrack now stores both educational records and curriculum planning artifacts. Security and privacy controls must cover not only student data, but also syllabus documents and generated year plans.

---

## Security objectives

1. Protect student and teacher data at rest.
2. Prevent accidental external transmission of syllabus or student data.
3. Keep planning and teaching features available offline.
4. Preserve auditability when plans are generated or changed.
5. Support school governance around regional calendar and curriculum use.

---

## Threat model summary

### Main assets
- student names and identifiers
- attendance history
- assessment evidence
- reports and advice
- syllabus source files
- extracted curriculum units
- year plans and planning notes

### Main threats
- unauthorised local access
- accidental export of sensitive files
- corrupted or partial plan generation
- hidden external calls from planning modules
- logging of sensitive syllabus/report content

---

## Security controls

### 1) Local encryption at rest
Apply to:
- database
- file attachments
- syllabus files
- report exports where feasible

### 2) App access control
- require app unlock on shared devices
- auto-lock after inactivity
- protect admin/settings screens if school requests it

### 3) IPC and process boundary security
- parser and LLM runtimes receive only required payloads
- reject arbitrary file-path injection
- use explicit temp directories for parsing/generation

### 4) Attachment handling security
- validate file type and size
- hash uploaded syllabus and evidence files
- isolate files by ownership path

### 5) Logging and diagnostics controls
- do not log raw student report text
- do not log raw syllabus content in production
- log IDs, statuses, timings, and summary counts only

### 6) Export controls
- separate student-report export from syllabus export
- label private teacher advice clearly
- prevent accidental inclusion of private notes

### 7) Plan integrity controls
- published plans are immutable
- regeneration creates a new version
- keep snapshot JSON of planning inputs for audit

---

## Privacy design principles

- local-first by default
- minimum necessary data
- clear separation between planning artifacts and student records
- teacher review before publish/export
- explainable data use in AI features

---

## Sensitive data handling rules

### Allowed in MVP
- syllabus file upload for planning
- extracted curriculum units
- attendance and assessment evidence
- editable report drafts

### Avoid by default in MVP
- external storage of raw student data
- external storage of raw syllabus content
- background syncing of planning documents

### Private teacher notes
Must remain excluded from student-facing outputs unless explicitly selected.

---

## Compliance posture

EduTrack should be able to support school policies around:
- educational record handling
- curriculum planning documentation
- retention and deletion
- export and audit

Actual compliance depends on school and jurisdiction, but the product design should not force unnecessary exposure.

---

## Operational controls for new planning features

### Syllabus upload
- only authorized user can upload/replace
- visible parse status and provenance
- preserve version history

### Calendar and region setup
- show source of holiday dates
- distinguish provider holiday vs manual school closure
- allow review before plan generation

### Year plan generation
- record inputs used
- store warnings produced
- prevent silent overwrite of published plans

---

## Incident response notes

Planning-specific incidents may include:
- wrong region selected causing bad pacing
- incomplete syllabus parse leading to weak plan
- accidental deletion of active syllabus file

Response should include:
- identify affected level/class
- freeze publish actions if needed
- restore previous published plan if available
- record corrective action in audit trail
