# 03 — Error handling, retries, and timeouts

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

EduTrack is used in live teaching contexts and now includes planning workflows that may run longer than normal CRUD actions. Error handling must preserve trust and prevent data loss.

---

## Error handling principles

1. Every error must be classified.
2. Planning failures must not block daily teaching workflows.
3. Long-running jobs must surface stage-based progress.
4. Published plans must never be silently overwritten.
5. Recovery paths should be obvious.

---

## Error taxonomy

### 1) Validation errors
Examples:
- level name empty
- academic year end before start
- class over 50 students
- no meeting days selected for year-plan generation
- report word count out of bounds

### 2) Not found errors
Examples:
- selected syllabus no longer exists
- calendar removed before plan generation
- year plan ID not found

### 3) Conflict/state errors
Examples:
- duplicate meeting-day rule
- trying to publish a failed plan
- year plan already published and immutable
- concurrent generation already running for same scope

### 4) Persistence errors
Examples:
- DB write failure for year plan lessons
- attachment metadata saved but file move failed

### 5) Parser/LLM runtime errors
Examples:
- syllabus parse timeout
- invalid structured extraction
- local model unavailable
- generation output fails schema validation

### 6) Security errors
Examples:
- disallowed file type
- file path escape attempt
- restricted export action

### 7) Unknown/internal errors
Catch-all with safe fallback messaging.

---

## Structured error model

```ts
type AppError = {
  code: string;
  message: string;
  field?: string | null;
  retryable: boolean;
  context?: {
    entityType?: string;
    entityId?: string;
    stage?: string;
  };
};
```

---

## User-facing message rules

- explain what failed
- explain what was preserved
- suggest the next safe action

Examples:
- "The syllabus uploaded, but extraction failed. Your file is saved and you can retry extraction."
- "This plan was already published. Create a new version to make changes."
- "No class days are selected yet, so the year plan cannot be generated."

---

## Retry policy

### Never retry automatically
- validation errors
- duplicate rule conflicts
- immutable published-plan conflicts
- unsupported file types

### Retry automatically with backoff
- transient file move failure
- temporary DB lock
- progress polling for running jobs

### Retry manually only
- syllabus extraction
- year-plan generation
- report generation after missing model/runtime issue

---

## Backoff defaults

- 250ms
- 750ms
- 1500ms
- max 3 retries

---

## Timeout policy

| Operation | Timeout |
|---|---|
| standard CRUD command | 5s |
| file move/storage | 15s |
| syllabus text extraction | 60s |
| syllabus LLM extraction | 120s |
| year-plan generation | 180s |
| report generation | 120s |

---

## Partial-failure handling

### Syllabus upload flow
If file store succeeds but DB row fails:
- remove orphaned file if possible
- show upload failed
- log diagnostic event

If DB row succeeds but extraction fails:
- keep syllabus row
- mark status failed
- allow retry extraction
- do not lose source file

### Year-plan generation flow
If plan header saves but lessons fail:
- roll back transaction where possible
- otherwise mark plan `failed`
- do not expose partial lesson set as valid in UI

### Daily session flow
If session saves but plan-link update fails:
- keep session
- show warning that plan progress was not updated
- allow retry of linking step later

---

## Offline and degraded mode

Degraded but usable states:
- planning disabled, attendance still works
- reports disabled, sessions/tests still work
- holiday auto-fill unavailable, manual calendar editing still works

---

## Logging guidance

Log:
- error code
- stage
- ids
- duration
- retry count

Do not log:
- raw syllabus text
- raw report text
- student names in production errors

---

## Key new failure modes to test

- duplicate class schedule rule
- academic year with zero teachable days after exclusions
- impossible syllabus completion due to too few class days
- plan generation returns malformed lesson dates
- regenerate-from-date conflicts with completed historical lessons
