# 05 — UX flows and wireframes

**Project:** EduTrack  
**Version:** 1.1

---

## Screen inventory

| Screen ID | Name | Entry points |
|---|---|---|
| S-01 | First launch / onboarding | Cold start, no existing data |
| S-02 | Daily dashboard | App open (default), sidebar "Today" |
| S-03 | Context selector | Top bar school / level / class chips |
| S-04 | Student roster | Sidebar "Students" |
| S-05 | Student profile | Tap student name on S-04 or S-02 |
| S-06 | Session editor | "Create lesson" on S-02 |
| S-07 | Assessment list | Sidebar "Tests" |
| S-08 | Assessment detail / scoring | Tap test on S-07 |
| S-09 | Agent configuration | Class settings → Agents |
| S-10 | Report generator | Sidebar "Generate" or S-05 |
| S-11 | Report editor | After generation completes |
| S-12 | School / level / class management | Sidebar "Structure" |
| S-13 | Built-in yearly calendar | Sidebar "Calendar" |
| S-14 | Syllabus upload and review | Level overview → Syllabus |
| S-15 | Class schedule setup | Level/Class planning flow |
| S-16 | Year plan review / publish | Sidebar "Planning" |
| S-17 | Settings | Sidebar "Settings" |

---

## Flow 1 — First launch and academic setup

```text
App open (no data)
  → S-01: Welcome screen
      "Welcome to EduTrack. Let's set up your school."
      [Create school]
  → Modal: School name + optional logo + region default
      [Save]
  → Modal: Create first level
      Name, code, academic year start/end, region
      [Save]
  → Modal: Upload syllabus for this level
      Drag and drop file
      [Upload now] | [Skip]
  → Modal: Create first class
      Name, subject
      [Save]
  → Modal: Choose class meeting days
      Mon Tue Wed Thu Fri Sat Sun toggles
      [Save]
  → Modal: Add your first student (or skip)
      [Add student] | [Go to planning]
  → S-16: Planning review if syllabus exists
      otherwise S-02 daily dashboard
```

**Empty states:**
- No school: full-screen welcome with one CTA.
- School exists, no levels: main area prompts "Add your first level/grade."
- Level exists, no syllabus: planning banner says "Upload syllabus to build the year."
- Class exists, no meeting days: planning banner says "Choose class days to generate a plan."
- Class exists, no students: roster panel says "No students yet — add your first student."

---

## Flow 2 — Level planning setup

This is the new planning backbone flow.

```text
Sidebar "Structure"
  → S-12 School / Level / Class management
      Select school
      Select level
      ↓
  S-14 Syllabus upload and review
      Upload syllabus file
      Wait for extraction
      Review extracted curriculum units
      [Approve extraction]
      ↓
  S-13 Built-in yearly calendar
      Confirm academic year dates
      Confirm region holidays
      Add manual closure days if needed
      [Save calendar]
      ↓
  S-15 Class schedule setup
      For each class in this level:
        choose 1 or many meeting days
      ↓
  S-16 Year plan review / publish
      [Generate year plan]
      Preview plan calendar + list
      See warnings if syllabus completion is tight
      [Publish plan]
```

---

## Flow 3 — Daily session (primary loop)

```text
App open
  → S-02 Daily dashboard
      Top bar shows:
        School chip → Level chip → Class chip
      Today's date and next planned lesson visible
      ↓
  Mark attendance
      Tap present / late / absent / excused per student
      OR tap "Mark all present"
      ↓
  Create or open today's lesson
      If planned lesson exists: open it
      Otherwise create an ad hoc session
      ↓
  Add notes / what was done
      Save session
      ↓
  Review insights
      Scan student/class insights
      ↓
  Done — switch class or close app
```

---

## Flow 4 — Add a test and score students

```text
S-02 Dashboard or Sidebar "Tests"
  → S-07 Assessment list
      [+ Add test]
  → Modal: New test
      Title, date, optional linked lesson
      [Save]
  → S-08 Assessment detail
      Student list with score input per row
      [Attach file] for evidence
```

---

## Flow 5 — View student profile

```text
Tap student name
  → S-05 Student profile
      Header: avatar, full name, nickname, level/grade, class
      Tabs: Overview | Attendance | Tests | Insights | Character
      ↓
  Overview
      Attendance summary
      score trend
      recent insights
      recent completed lessons
```

---

## Flow 6 — Generate a student report

```text
S-05 Student profile → [Generate report]
  OR Sidebar "Generate"
  → S-10 Report generator
      Student, class, level shown
      Word count target
      Evidence summary
      Optional toggle: include curriculum/year-plan context
      [Generate report]
  → S-11 Report editor
      [School report]
      [Teacher advice]
      [Save draft] [Export]
```

---

## Flow 7 — Regenerate planning after disruption

```text
S-16 Year plan review
  → Warning banner:
      "3 classes lost 2 teaching days this month."
  → [Regenerate from date]
      Select start date
      Preview downstream lesson shifts
      [Apply]
```

---

## Panel layout — daily dashboard (S-02)

```text
┌────────────────────────────────────────────────────────────────────┐
│ Sidebar      │ Topbar: School > Level > Class + today + plan chip │
│ [Today]      ├─────────────────────────────────────────────────────┤
│ [Students]   │ Stat cards: Students | Present | Next lesson | Plan │
│ [Calendar]   ├───────────────────────┬─────────────────────────────┤
│ [Planning]   │ Attendance            │ Today lesson / session log  │
│ [Tests]      │                       ├─────────────────────────────┤
│ [Generate]   │ student rows          │ AI insights                 │
│ [Structure]  │ + status buttons      ├─────────────────────────────┤
│ [Settings]   │                       │ Progress / coverage         │
└────────────────────────────────────────────────────────────────────┘
```

---

## Panel layout — year plan review (S-16)

```text
┌────────────────────────────────────────────────────────────────────┐
│ Header: School / Level / Class / Plan status / Publish button     │
├──────────────────────────────┬─────────────────────────────────────┤
│ Left                         │ Right                               │
│ Calendar month view          │ Lesson sequence table               │
│ - class days highlighted     │ - date                              │
│ - holidays crossed out       │ - unit                              │
│ - buffer days marked         │ - title                             │
│ - skipped days dimmed        │ - status                            │
├──────────────────────────────┴─────────────────────────────────────┤
│ Footer: warnings + capacity summary + regenerate controls          │
└────────────────────────────────────────────────────────────────────┘
```

---

## Responsive behaviour

| Viewport | Layout |
|---|---|
| ≥ 1200px | Full dashboard and split planning views |
| 900–1199px | Split views compress to stacked cards |
| 800–899px | Sidebar collapses to icon-only |
| < 800px | Out of MVP scope |

---

## Interaction micro-patterns

### Meeting-day picker
- weekday chips toggle on/off
- selected days show filled state
- summary text updates live: "Class meets Mon, Wed, Fri"

### Syllabus upload
- drag/drop area shows file type and size
- parse status updates inline: uploaded → parsing → extracted → review required

### Year plan generation
- progress states:
  1. reading syllabus
  2. resolving holidays
  3. calculating available class days
  4. generating pacing
  5. validating and saving

### Planned vs actual lesson
- planned lesson card shows linked status
- after session completion, user sees either:
  - "Completed as planned"
  - "Completed with edits"
  - "Not linked to plan"
