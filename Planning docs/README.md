# EduTrack — Phase 2–5 document pack

## Version
**Updated:** 1.1  
**Scope update:** school → level/grade → class → student, built-in yearly calendar, level syllabus upload, AI-assisted syllabus-to-year planning

## Included docs

### Phase 2 — design & architecture
- 05_Design_System_And_Accessibility.md
- 05_UX_Flows_And_Wireframes.md
- 03_System_Architecture_Overview.md
- 03_Tech_Stack_Lock.md
- 03_Component_Design_Modules_And_Interfaces.md
- 03_Functions_Objects_And_Attributes.md

### Phase 3 — data & API
- 04_Data_Plan.md
- 04_Database_Schema_And_Migrations.md
- 03_API_Contracts_And_Schemas.md
- 03_Error_Handling_Retries_Timeouts.md

### Phase 4 — safety & config
- 07_Security_Privacy_Compliance_Plan.md
- 07_Data_Privacy_DPIA_Notes.md
- 06_Configuration_And_Feature_Flags.md

### Phase 5 — delivery
- 08_QA_Test_Strategy.md
- 01_Success_Metrics_Analytics_Plan.md
- 10_Deployment_Architecture.md

## What changed in this update
- Core hierarchy is now **school → level/grade → class → student**.
- Added a built-in **yearly calendar** with region-aware holiday handling.
- Added **syllabus upload per level**.
- Added **class meeting-day rules** so classes can meet one or more times per week.
- Added **AI-assisted yearly pacing generation** that maps syllabus coverage across the available teaching days for a selected region and academic year.
- Updated planning, UX, data, API, QA, security, deployment, and analytics docs to reflect these additions.

## Notes
- The original visual assets (`edutrack_app_architecture.svg`, `edutrack_colorful_dashboard.html`) are included unchanged in the replacement pack because the request focused on planning docs.
- These markdown files are written as full drop-in replacements for the existing Planning docs folder.
