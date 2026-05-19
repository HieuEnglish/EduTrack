# 07 — Data privacy DPIA notes

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

These notes extend the privacy assessment for the new level planning features.

---

## Processing summary

### Processing activity
Local classroom management, syllabus ingestion, yearly planning, and report generation for teachers.

### Data subjects
- students
- teachers
- possibly school staff named in documents

### Data types
- student names and learning records
- attendance data
- assessment data
- uploaded syllabus documents
- extracted curriculum structures
- generated reports and teacher advice

### Processing methods
- local storage
- local file parsing
- local AI analysis and report generation
- local calendar/holiday computation

### Processing locations
- teacher device by default

---

## Purpose and necessity

### Core purposes
- track progress and attendance
- manage classes within levels/grades
- upload and structure a syllabus
- build a realistic year plan for syllabus completion
- support reporting and intervention

### Necessity rationale
The planning feature reduces manual teacher workload and makes lesson pacing more consistent. Local processing reduces exposure compared with cloud-based curriculum tools.

### Proportionality
Only one active syllabus per level is needed for planning. Planning should not require uploading unnecessary student data.

---

## Risks to data subjects

### Risk 1 — device loss or theft
Mitigation:
- local encryption
- app lock
- no external sync by default

### Risk 2 — over-collection of personal data
Mitigation:
- keep syllabus separate from student records
- avoid adding student data to planning flows unless needed
- limit diagnostics to summary metadata

### Risk 3 — accidental disclosure through exports
Mitigation:
- separate planning exports from student exports
- clear export labels
- private advice excluded by default

### Risk 4 — AI-generated inaccuracies
Mitigation:
- extracted syllabus units require review
- generated plans are drafts before publish
- generated reports remain editable

### Risk 5 — hidden third-party transmission
Mitigation:
- local-first runtime
- visible provider settings
- no silent remote syllabus upload

### Risk 6 — sensitive notes appear in student-facing outputs
Mitigation:
- private notes excluded from standard exports
- separate school report and private advice

### Risk 7 — incorrect region/holiday assumptions affect instruction
Mitigation:
- show region source clearly
- allow manual closure overrides
- require user review before publish

---

## Lawful basis / institutional basis notes

The exact lawful basis depends on school jurisdiction, but EduTrack should support common school administration and educational record handling bases. Core operations should not depend on blanket consent where a school has a more appropriate institutional basis.

---

## Transparency requirements

The product should make it easy to answer:
- what data is stored for a student
- what syllabus file is active for a level
- which evidence sources were used for a report
- which dates and holidays were used for a year plan
- when a plan was generated and with which model/runtime version

---

## Retention notes

Suggested prompts for schools:
- how long should archived syllabus versions be kept?
- how long should published year plans remain available?
- when should archived students be export-and-purged?

Default product posture:
- retain until teacher/school archives or deletes
- preserve published plan history for auditability
