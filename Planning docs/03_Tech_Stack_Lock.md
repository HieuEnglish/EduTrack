# 03 — Tech stack lock

**Project:** EduTrack  
**Version:** 1.1

---

## Locked decisions

The stack remains local-first and desktop-first. New planning features must fit the existing product direction.

---

## Desktop shell
- **Tauri** — desktop packaging and IPC boundary

## UI framework
- **React + TypeScript**
- **zustand** — UI state such as selected school/level/class and panel states

## App core language
- **Rust**
- planning, validation, repositories, and calendar logic remain in Rust

## Database
- **SQLite**
- one database file per teacher/device
- suitable for hierarchical planning and operational history

## LLM engine
- local model/runtime only in MVP
- used for syllabus extraction, year-plan generation, and reports

## File storage
- local filesystem under app data directory
- syllabus files, evidence files, and exports live here

## PDF export
- existing PDF export approach retained for reports

## Calendar/holiday logic
- implemented in Rust service layer
- no hard dependency on always-online APIs in MVP
- region holiday data should be snapshotted locally into calendar closure rows

## Build and distribution
- existing desktop packaging approach retained

## Development toolchain
- TypeScript, Rust, SQLite migration tooling, local test fixtures

## What is explicitly excluded
- cloud-only planning
- mandatory online holiday service dependency
- browser/mobile-first architecture in MVP
