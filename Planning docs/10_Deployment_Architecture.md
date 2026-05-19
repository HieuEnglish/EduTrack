# 10 — Deployment architecture

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

This document describes how the updated EduTrack app is installed and run with the new planning modules.

---

## Deployment model summary

EduTrack remains a desktop application installed on a teacher-managed or school-managed device. The planning features add:
- syllabus file storage
- parser support
- calendar/holiday logic
- larger local data footprints for year plans

---

## Runtime topology

```text
Desktop App (Tauri)
  ├─ React UI
  ├─ Rust app core
  ├─ SQLite DB
  ├─ local file storage
  ├─ parser worker
  └─ local LLM runtime (optional but required for AI planning/report features)
```

---

## Installation targets

### Managed school devices
- supported when local file storage and model runtime can be provisioned
- suitable for consistent region/calendar defaults

### Individual teacher devices
- supported for solo usage
- teacher manages syllabus uploads and local runtime

---

## Packaging

### Desktop package formats
- keep existing package targets
- include migrations for new planning tables
- optionally bundle parser helpers

---

## Installation flow

### First install
- create app data directories
- initialize SQLite DB
- apply migrations
- create file storage folders for syllabus/evidence/exports

### Environment checks
- writable storage
- available DB path
- parser helper available
- local runtime available for AI planning/report features

---

## Directory layout (conceptual)

```text
{app_data_dir}/
  db/edutrack.sqlite
  files/
    syllabus/{level_id}/
    assessments/{class_id}/
    reports/{student_id}/
  temp/
    parsing/
    generation/
  logs/
```

---

## Update architecture

### App updates
- preserve DB and files
- run additive migrations
- maintain plan history

### Migration-safe update rules
- never remove old class/session/report data during planning migrations
- backfill default levels where required
- keep old records readable after hierarchy upgrade

### Model/runtime updates
- app still usable without local runtime, but planning/report generation disabled with clear UI warning

---

## Deployment variants

### Variant A — core app only
- hierarchy, calendar, manual planning, attendance, assessments
- no AI extraction/generation

### Variant B — app + parser helper
- syllabus upload and extraction preprocessing
- no LLM year-plan or report generation without runtime

### Variant C — app + parser + local LLM runtime
- full planning and reporting experience

---

## Backup and restore considerations

Back up:
- SQLite DB
- syllabus files
- assessment attachments
- exports if required

Restore should preserve:
- hierarchy references
- calendar snapshots
- published year plans
- historical sessions and reports

---

## Acceptance checks for deployment

- upgrade works from previous version
- default levels backfilled successfully
- classes appear under correct levels
- syllabus upload directory created
- planning features gracefully disable if parser/runtime unavailable
