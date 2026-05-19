# 03 — API contracts and schemas

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

This document defines the application-facing contracts for the updated EduTrack model, including:
- school → level → class → student hierarchy
- syllabus upload and extraction
- yearly calendar and holiday-aware scheduling
- AI-assisted year-plan generation

---

## Contract principles

1. Commands mutate state; queries return read models.
2. IDs are opaque UUIDs.
3. Year plans are versioned. Published plans are immutable.
4. Long-running tasks return job/progress states.
5. Hierarchy ownership is explicit in every payload where ambiguity could exist.

---

## Global schema conventions

### IDs
```ts
type Uuid = string;
```

### Dates and times
```ts
type IsoDate = string;      // YYYY-MM-DD
type IsoDateTime = string;  // ISO 8601
type Weekday = "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun";
```

### Envelope pattern
```ts
type ApiResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: AppError };
```

---

## Common types

```ts
type PlanningStatus = "draft" | "ready" | "active" | "archived";
type ParserStatus = "pending" | "parsed" | "failed" | "reviewed";
type GenerationStatus = "draft" | "generated" | "reviewed" | "published" | "superseded" | "failed";
type AttendanceStatus = "present" | "late" | "absent" | "excused";
type AgentType = "attendance" | "literacy" | "numeracy" | "participation" | "writing";
type PlanScope = "level_default" | "class_specific";
```

```ts
type AppError = {
  code: string;
  message: string;
  field?: string | null;
  retryable?: boolean;
};
```

---

## Read models returned to UI

### `SchoolSummary`
```ts
type SchoolSummary = {
  id: Uuid;
  name: string;
  levelCount: number;
  classCount: number;
  studentCount: number;
  regionCode: string | null;
};
```

### `LevelSummary`
```ts
type LevelSummary = {
  id: Uuid;
  schoolId: Uuid;
  name: string;
  code: string | null;
  displayOrder: number;
  academicYearStart: IsoDate;
  academicYearEnd: IsoDate;
  regionCode: string;
  syllabusStatus: ParserStatus | "none";
  planningStatus: PlanningStatus;
  classCount: number;
};
```

### `ClassSummary`
```ts
type ClassSummary = {
  id: Uuid;
  schoolId: Uuid;
  levelId: Uuid;
  name: string;
  subjectName: string | null;
  studentCount: number;
  schedulePatternSummary: string | null;
  yearPlanStatus: GenerationStatus | "none";
};
```

### `StudentSummary`
```ts
type StudentSummary = {
  id: Uuid;
  schoolId: Uuid;
  levelId: Uuid;
  classId: Uuid;
  fullName: string;
  preferredName: string | null;
  attendanceRate: number | null;
  averageScore: number | null;
  character: {
    nickname: string;
    levelCurrent: number;
    levelMax: number;
  };
};
```

### `AcademicCalendarView`
```ts
type AcademicCalendarView = {
  id: Uuid;
  schoolId: Uuid;
  levelId: Uuid | null;
  name: string;
  academicYearStart: IsoDate;
  academicYearEnd: IsoDate;
  regionCode: string;
  teachableDays: number;
  holidayDays: number;
  manualClosureDays: number;
  isDefault: boolean;
};
```

### `SyllabusSummary`
```ts
type SyllabusSummary = {
  id: Uuid;
  levelId: Uuid;
  title: string;
  versionLabel: string | null;
  parserStatus: ParserStatus;
  llmExtractionStatus: "not_started" | "running" | "completed" | "failed";
  unitCount: number;
  isActive: boolean;
};
```

### `CurriculumUnitView`
```ts
type CurriculumUnitView = {
  id: Uuid;
  syllabusId: Uuid;
  unitCode: string | null;
  title: string;
  description: string | null;
  recommendedSequence: number;
  estimatedLessons: number | null;
  estimatedWeeks: number | null;
  isRequired: boolean;
};
```

### `YearPlanSummary`
```ts
type YearPlanSummary = {
  id: Uuid;
  schoolId: Uuid;
  levelId: Uuid;
  classId: Uuid | null;
  syllabusId: Uuid;
  calendarId: Uuid;
  regionCode: string;
  planScope: PlanScope;
  generationStatus: GenerationStatus;
  totalTeachingDays: number;
  totalPlannedLessons: number;
  holidayDaysExcluded: number;
  bufferDaysReserved: number;
  generatedAt: IsoDateTime | null;
  publishedAt: IsoDateTime | null;
};
```

### `YearPlanLessonView`
```ts
type YearPlanLessonView = {
  id: Uuid;
  yearPlanId: Uuid;
  teachingDate: IsoDate;
  weekday: Weekday;
  sequenceNumber: number;
  curriculumUnitId: Uuid | null;
  lessonTitle: string;
  lessonObjective: string | null;
  isBuffer: boolean;
  isHolidayAdjusted: boolean;
  status: "planned" | "completed" | "skipped" | "moved";
};
```

### `StudentReportView`
```ts
type StudentReportView = {
  id: Uuid;
  studentId: Uuid;
  classId: Uuid;
  generatedFromPlanId: Uuid | null;
  wordCountTarget: number;
  reportText: string;
  teacherAdviceText: string | null;
  generatedAt: IsoDateTime;
};
```

---

## Command catalogue

### School commands

#### `create_school`
```ts
type CreateSchoolInput = {
  name: string;
  logoPath?: string | null;
  countryCode?: string | null;
  regionCode?: string | null;
  timezone?: string | null;
};

type CreateSchoolOutput = {
  school: SchoolSummary;
};
```

#### `list_schools`
```ts
type ListSchoolsOutput = {
  schools: SchoolSummary[];
};
```

### Level commands

#### `create_level`
```ts
type CreateLevelInput = {
  schoolId: Uuid;
  name: string;
  code?: string | null;
  displayOrder?: number;
  academicYearStart: IsoDate;
  academicYearEnd: IsoDate;
  regionCode: string;
  timezone?: string | null;
};

type CreateLevelOutput = {
  level: LevelSummary;
};
```

#### `list_levels_by_school`
```ts
type ListLevelsBySchoolInput = {
  schoolId: Uuid;
};

type ListLevelsBySchoolOutput = {
  levels: LevelSummary[];
};
```

#### `update_level`
```ts
type UpdateLevelInput = {
  levelId: Uuid;
  name?: string;
  code?: string | null;
  displayOrder?: number;
  academicYearStart?: IsoDate;
  academicYearEnd?: IsoDate;
  regionCode?: string;
};
```

### Calendar commands

#### `create_academic_calendar`
```ts
type CreateAcademicCalendarInput = {
  schoolId: Uuid;
  levelId?: Uuid | null;
  name: string;
  academicYearStart: IsoDate;
  academicYearEnd: IsoDate;
  regionCode: string;
  makeDefault?: boolean;
};

type CreateAcademicCalendarOutput = {
  calendar: AcademicCalendarView;
};
```

#### `list_calendars`
```ts
type ListCalendarsInput = {
  schoolId: Uuid;
  levelId?: Uuid | null;
};

type ListCalendarsOutput = {
  calendars: AcademicCalendarView[];
};
```

#### `add_manual_closure_day`
```ts
type AddManualClosureDayInput = {
  calendarId: Uuid;
  date: IsoDate;
  reason?: string | null;
};
```

### Syllabus commands

#### `upload_syllabus`
```ts
type UploadSyllabusInput = {
  levelId: Uuid;
  title: string;
  versionLabel?: string | null;
  attachmentPath: string;
  makeActive?: boolean;
};

type UploadSyllabusOutput = {
  syllabus: SyllabusSummary;
};
```

#### `get_syllabus_extraction`
```ts
type GetSyllabusExtractionInput = {
  syllabusId: Uuid;
};

type GetSyllabusExtractionOutput = {
  syllabus: SyllabusSummary;
  units: CurriculumUnitView[];
};
```

#### `review_syllabus_extraction`
```ts
type ReviewSyllabusExtractionInput = {
  syllabusId: Uuid;
  units: CurriculumUnitView[];
  markReviewed: boolean;
};
```

### Class commands

#### `create_class`
```ts
type CreateClassInput = {
  schoolId: Uuid;
  levelId: Uuid;
  name: string;
  subjectName?: string | null;
  academicYearLabel?: string | null;
};

type CreateClassOutput = {
  class: ClassSummary;
};
```

#### `list_classes_by_level`
```ts
type ListClassesByLevelInput = {
  levelId: Uuid;
};

type ListClassesByLevelOutput = {
  classes: ClassSummary[];
};
```

#### `set_class_schedule_rules`
```ts
type SetClassScheduleRulesInput = {
  classId: Uuid;
  rules: Array<{
    weekday: Weekday;
    startTime?: string | null;
    endTime?: string | null;
    periodLabel?: string | null;
  }>;
};

type SetClassScheduleRulesOutput = {
  class: ClassSummary;
};
```

#### `update_class_agents`
```ts
type UpdateClassAgentsInput = {
  classId: Uuid;
  agents: Array<{
    agentType: AgentType;
    enabled: boolean;
  }>;
};
```

### Year plan commands

#### `generate_year_plan`
```ts
type GenerateYearPlanInput = {
  levelId: Uuid;
  classId?: Uuid | null;
  syllabusId?: Uuid | null;
  calendarId?: Uuid | null;
  bufferDaysPercent?: number | null;
  pacingMode?: "balanced" | "front_loaded" | "back_loaded";
};

type GenerateYearPlanOutput = {
  yearPlan: YearPlanSummary;
  warnings: string[];
};
```

#### `get_year_plan`
```ts
type GetYearPlanInput = {
  yearPlanId: Uuid;
};

type GetYearPlanOutput = {
  yearPlan: YearPlanSummary;
  lessons: YearPlanLessonView[];
};
```

#### `publish_year_plan`
```ts
type PublishYearPlanInput = {
  yearPlanId: Uuid;
};

type PublishYearPlanOutput = {
  yearPlan: YearPlanSummary;
};
```

#### `regenerate_year_plan_from_date`
```ts
type RegenerateYearPlanFromDateInput = {
  yearPlanId: Uuid;
  fromDate: IsoDate;
};

type RegenerateYearPlanFromDateOutput = {
  yearPlan: YearPlanSummary;
  lessons: YearPlanLessonView[];
};
```

### Student commands

#### `add_student`
```ts
type AddStudentInput = {
  classId: Uuid;
  fullName: string;
  preferredName?: string | null;
  studentCode?: string | null;
};

type AddStudentOutput = {
  student: StudentSummary;
};
```

#### `list_students_by_class`
```ts
type ListStudentsByClassInput = {
  classId: Uuid;
};

type ListStudentsByClassOutput = {
  students: StudentSummary[];
};
```

### Session and attendance commands

#### `create_session`
```ts
type CreateSessionInput = {
  classId: Uuid;
  sessionDate: IsoDate;
  title: string;
  description?: string | null;
  topicTags?: string[];
  yearPlanLessonId?: Uuid | null;
};
```

#### `mark_attendance`
```ts
type MarkAttendanceInput = {
  sessionId: Uuid;
  studentId: Uuid;
  status: AttendanceStatus;
  note?: string | null;
};
```

### Report commands

#### `generate_report`
```ts
type GenerateReportInput = {
  studentId: Uuid;
  wordCountTarget: number;
  includePlanContext?: boolean;
};

type GenerateReportOutput = {
  report: StudentReportView;
};
```

---

## Long-running job/progress model

Used for syllabus extraction and year-plan generation.

```ts
type JobProgressView = {
  jobId: Uuid;
  jobType: "syllabus_extraction" | "year_plan_generation" | "report_generation";
  status: "queued" | "running" | "completed" | "failed";
  progressPercent: number;
  stageLabel: string;
  errorMessage?: string | null;
};
```

---

## Notes on backwards compatibility

Compared with the previous version:
- `ClassSummary` now includes `levelId`
- `StudentSummary` now includes `schoolId` and `levelId`
- classes are no longer listed directly under school in primary flows
- new planning resources (`AcademicCalendar`, `Syllabus`, `YearPlan`) are first-class API surfaces
