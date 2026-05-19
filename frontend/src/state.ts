import type {
  School, Level, ClassView, SchoolDirectory, Student, SessionView, AttendanceView,
  AgentInsight, ScheduleRule, CalendarClosureDay,
  YearPlanBundle, LlmConfig, LlmModelOption, GradingDiagnostics, TestTemplate, StudentSubmission, CalendarClassSessionView,
  StudentReportStatus, TeacherProfile, AppState,
} from './types';
import {
  invoke, parseYmdUtc, formatYmdUtc, weekdayToJsDay, academicYearWindow,
  browserTimezone,
} from './utils';

export { invoke };

export const holidaySyncAttemptedLevels = new Set<string>();

export const state: AppState = createAppState();

function createAppState(): AppState {
  return {
    page: 'overview',
    classTab: 'students',
    calendarMode: 'month',
    classCalendarMode: 'month',
    classCalendarMonth: currentMonthKey(),
    showAttendanceHistory: false,
    backend: false,
    loading: true,
    message: '',
    schools: [],
    levels: [],
    classes: [],
    students: [],
    sessions: [],
    attendanceRecords: [],
    scheduleRules: [],
    insights: [],
    extractedUnits: [],
    syllabusExtractionDiagnostics: null,
    generatedLessons: [],
    currentSyllabusId: '',
    lessonExportScope: 'week',
    lessonExportFormat: 'html',
    lessonTemplate: '',
    lessonTemplateName: '',
    lessonContextNotes: loadContextNotes(),
    calendarClosures: [],
    planMessage: '',
    syllabusBusy: false,
    lessonPlansBusy: false,
    detailedPlanProgressActive: false,
    detailedPlanProgressTotal: 0,
    detailedPlanProgressCompleted: 0,
    detailedPlanProgressMessage: '',
    setupNotice: '',
    setupBusy: '',
    directory: [],
    selectedSchoolId: '',
    selectedLevelId: '',
    selectedClassId: '',
    llmConfig: null,
    llmModels: [],
    gradingDiagnostics: null,
    gradingToolsInstallBusy: false,
    gradingToolInstallNotes: {},
    teacherProfile: loadTeacherProfile(),
    tests: [],
    currentTestId: '',
    testSubmissions: [],
    classroomTestSubmissions: {},
    reportStatuses: [],
    reportInstructions: '',
    reportWordCount: 300,
    allCalendarLessons: [],
    allCalendarSessions: [],
  };
}

export function selectedSchool(state: AppState) {
  return state.schools.find((school) => school.id === state.selectedSchoolId) ?? null;
}

export function selectedLevel(state: AppState) {
  return state.levels.find((level) => level.id === state.selectedLevelId) ?? null;
}

export function selectedClass(state: AppState) {
  return state.classes.find((klass) => klass.id === state.selectedClassId) ?? null;
}

export function loadTeacherProfile(): TeacherProfile {
  try {
    const raw = localStorage.getItem('edutrack.teacherProfile');
    if (!raw) return { name: 'Teacher', title: 'Connected', image: null };
    const parsed = JSON.parse(raw) as Partial<TeacherProfile>;
    return {
      name: parsed.name?.trim() || 'Teacher',
      title: parsed.title?.trim() || 'Teacher workspace',
      image: parsed.image || null,
    };
  } catch {
    return { name: 'Teacher', title: 'Teacher workspace', image: null };
  }
}

export function saveTeacherProfile(profile: TeacherProfile) {
  localStorage.setItem('edutrack.teacherProfile', JSON.stringify(profile));
}

function loadContextNotes(): string {
  try { return localStorage.getItem('edutrack.lessonContextNotes') ?? ''; } catch { return ''; }
}

function currentMonthKey() {
  const now = new Date();
  return `${now.getFullYear()}-${`${now.getMonth() + 1}`.padStart(2, '0')}`;
}

export async function loadSchoolDirectory(state: AppState) {
  if (!state.backend || state.schools.length === 0) {
    state.directory = [];
    return;
  }
  const directory: SchoolDirectory[] = [];
  for (const school of state.schools) {
    const levels = (await invoke<Level[]>('get_levels_by_school', { schoolId: school.id })) ?? [];
    const levelsWithClasses = [];
    for (const level of levels) {
      const classes = (await invoke<ClassView[]>('get_classes_by_level', { levelId: level.id })) ?? [];
      levelsWithClasses.push({ ...level, classes });
    }
    directory.push({ school, levels: levelsWithClasses });
  }
  state.directory = directory;
}

export async function loadHierarchy(state: AppState) {
  if (!state.selectedSchoolId) {
    state.levels = [];
    state.classes = [];
    state.students = [];
    return;
  }
  state.levels = (await invoke<Level[]>('get_levels_by_school', { schoolId: state.selectedSchoolId })) ?? [];
  if (!state.levels.some((level) => level.id === state.selectedLevelId)) {
    state.selectedLevelId = state.levels[0]?.id ?? '';
  }
  state.classes = state.selectedLevelId
    ? ((await invoke<ClassView[]>('get_classes_by_level', { levelId: state.selectedLevelId })) ?? [])
    : [];
  if (!state.classes.some((klass) => klass.id === state.selectedClassId)) {
    state.selectedClassId = state.classes[0]?.id ?? '';
  }
  await loadClassData(state);
  await loadCalendarClosures(state);
}

export async function refreshSelectedHierarchy(state: AppState, schoolId: string, levelId?: string, classId?: string) {
  state.schools = (await invoke<School[]>('get_schools')) ?? [];
  await loadSchoolImages(state);
  state.selectedSchoolId = schoolId || state.selectedSchoolId || state.schools[0]?.id || '';
  state.levels = state.selectedSchoolId
    ? ((await invoke<Level[]>('get_levels_by_school', { schoolId: state.selectedSchoolId })) ?? [])
    : [];
  state.selectedLevelId =
    levelId && state.levels.some((level) => level.id === levelId)
      ? levelId
      : state.levels.some((level) => level.id === state.selectedLevelId)
        ? state.selectedLevelId
        : state.levels[0]?.id || '';
  state.classes = state.selectedLevelId
    ? ((await invoke<ClassView[]>('get_classes_by_level', { levelId: state.selectedLevelId })) ?? [])
    : [];
  state.selectedClassId =
    classId && state.classes.some((klass) => klass.id === classId)
      ? classId
      : state.classes.some((klass) => klass.id === state.selectedClassId)
        ? state.selectedClassId
        : state.classes[0]?.id || '';
  await loadClassData(state);
  await loadCalendarClosures(state);
  await loadSchoolDirectory(state);
}

export async function loadClassData(state: AppState) {
  if (!state.selectedClassId) {
    state.students = [];
    state.sessions = [];
    state.attendanceRecords = [];
    state.scheduleRules = [];
    state.insights = [];
    state.generatedLessons = [];
    state.tests = [];
    state.currentTestId = '';
    state.testSubmissions = [];
    state.classroomTestSubmissions = {};
    state.reportStatuses = [];
    return;
  }
  state.students = (await invoke<Student[]>('get_students_by_class', { classId: state.selectedClassId })) ?? [];
  state.sessions = (await invoke<SessionView[]>('get_sessions_by_class', { classId: state.selectedClassId })) ?? [];
  await loadAttendanceData(state);
  const klass = selectedClass(state);
  if (klass?.yearPlanId) {
    const plan = await invoke<YearPlanBundle>('get_year_plan', { yearPlanId: klass.yearPlanId });
    state.generatedLessons = plan?.lessons ?? [];
  } else {
    state.generatedLessons = [];
  }
  state.scheduleRules = (await invoke<ScheduleRule[]>('get_class_schedule_rules', { classId: state.selectedClassId })) ?? [];
  state.tests = (await invoke<TestTemplate[]>('get_class_tests', { classId: state.selectedClassId })) ?? [];
  state.classroomTestSubmissions = await loadClassroomTestSubmissions(state.tests);
  state.reportStatuses = (await invoke<StudentReportStatus[]>('get_class_report_status', { classId: state.selectedClassId })) ?? [];
  await runAgentsForCurrentClass(state);
}

async function loadClassroomTestSubmissions(tests: TestTemplate[]): Promise<Record<string, StudentSubmission[]>> {
  if (tests.length === 0) return {};
  const pairs = await Promise.all(
    tests.map(async (test) => {
      const request = invoke<StudentSubmission[]>('get_test_submissions', { assessmentId: test.id });
      const submissions = request ? (await request) ?? [] : [];
      return [test.id, submissions] as const;
    }),
  );
  const matrix: Record<string, StudentSubmission[]> = {};
  for (const [testId, submissions] of pairs) matrix[testId] = submissions;
  return matrix;
}

export async function runAgentsForCurrentClass(state: AppState) {
  if (!state.backend || !state.selectedClassId) {
    state.insights = [];
    return;
  }
  try {
    state.insights = (await invoke<AgentInsight[]>('run_class_agents', { classId: state.selectedClassId })) ?? [];
  } catch {
    state.insights = [];
  }
}

export async function loadAttendanceData(state: AppState) {
  if (!state.backend || state.sessions.length === 0) {
    state.attendanceRecords = [];
    return;
  }
  const batches = await Promise.all(
    state.sessions.map((session) => invoke<AttendanceView[]>('get_attendance_by_session', { sessionId: session.id })),
  );
  state.attendanceRecords = batches.flatMap((records) => records ?? []);
}

export async function ensureAttendanceSessionsForSelectedClass(state: AppState) {
  const klass = selectedClass(state);
  const level = selectedLevel(state);
  if (!state.backend || !klass || !level || state.scheduleRules.length === 0) return 0;

  const closures = await ensurePublicHolidaysForSelectedLevel(state);
  const closureDates = new Set(closures.map((c) => c.closureDate));
  const range = classDateRange(klass, level);
  const sessionDates = datesForWeekdays(range.start, range.end, state.scheduleRules.map((r) => r.weekday))
    .filter((date) => !closureDates.has(date));
  const existingDates = new Set(state.sessions.map((s) => s.sessionDate));
  const missingDates = sessionDates.filter((date) => !existingDates.has(date));
  if (missingDates.length === 0) return 0;

  for (const date of missingDates) {
    await invoke<string>('create_session', {
      classId: klass.id, yearPlanLessonId: null, sessionDate: date,
      title: `${klass.name} class`, description: 'Auto-created from the class meeting days.',
      topicTagsJson: '[]',
    });
  }
  state.sessions = (await invoke<SessionView[]>('get_sessions_by_class', { classId: klass.id })) ?? [];
  await loadAttendanceData(state);
  return missingDates.length;
}

export async function ensurePublicHolidaysForSelectedLevel(state: AppState) {
  const level = selectedLevel(state);
  if (!state.backend || !level) return state.calendarClosures;
  if (holidaySyncAttemptedLevels.has(level.id)) {
    if (state.calendarClosures.length === 0 && level.defaultCalendarId) await loadCalendarClosures(state);
    return state.calendarClosures;
  }
  try {
    const { closures } = await syncPublicHolidaysForLevel(state, level);
    holidaySyncAttemptedLevels.add(level.id);
    return closures;
  } catch (error) {
    if (level.defaultCalendarId) await loadCalendarClosures(state);
    console.warn('Holiday sync failed; using saved closure days only.', error);
    return state.calendarClosures;
  }
}

export async function loadCalendarClosures(state: AppState) {
  const level = selectedLevel(state);
  if (!state.backend || !level?.defaultCalendarId) { state.calendarClosures = []; return; }
  state.calendarClosures = (await invoke<CalendarClosureDay[]>('get_calendar_closure_days', { calendarId: level.defaultCalendarId })) ?? [];
}

export async function loadSettings(state: AppState) {
  const config = await invoke<LlmConfig>('get_local_model_config');
  state.llmConfig = config ?? null;
  if (state.llmConfig) await loadModels(state, state.llmConfig.provider);
  const diagnosticsCall = invoke<GradingDiagnostics>('get_grading_diagnostics');
  state.gradingDiagnostics = diagnosticsCall ? await diagnosticsCall.catch(() => null) : null;
}

export async function loadModels(state: AppState, provider: string) {
  state.llmModels = (await invoke<LlmModelOption[]>('list_llm_models', { provider })) ?? [];
}

export async function loadSchoolImages(state: AppState) {
  if (!state.backend) return;
  for (const school of state.schools) {
    if (school.logoPath && !school.logoPath.startsWith('data:')) {
      try { school.logoPath = await invoke<string>('read_image_file', { path: school.logoPath }); }
      catch { school.logoPath = null; }
    }
  }
}

export async function loadAllCalendarLessons(state: AppState) {
  if (!state.backend) { state.allCalendarLessons = []; return; }
  try {
    const lessons = await invoke<import('./types').CalendarLessonView[]>('get_all_calendar_lessons');
    state.allCalendarLessons = lessons ?? [];
  } catch {
    state.allCalendarLessons = [];
  }
}

export async function loadAllCalendarSessions(state: AppState) {
  if (!state.backend) { state.allCalendarSessions = []; return; }
  try {
    const sessions = await invoke<CalendarClassSessionView[]>('get_all_calendar_sessions');
    state.allCalendarSessions = sessions ?? [];
  } catch {
    state.allCalendarSessions = [];
  }
}

export async function syncPublicHolidaysForLevel(state: AppState, level: Level) {
  const calendarId = await ensureCalendarForLevel(state, level);
  const countryCode = (level.regionCode || findSchoolRegion(state, level.schoolId)).toUpperCase();
  const closures = (await invoke<CalendarClosureDay[]>('sync_public_holidays', {
    calendarId, countryCode,
    academicYearStart: level.academicYearStart, academicYearEnd: level.academicYearEnd,
  })) ?? [];
  state.calendarClosures = closures;
  holidaySyncAttemptedLevels.add(level.id);
  return { calendarId, closures };
}

export async function ensureCalendarForLevel(state: AppState, level: Level) {
  if (level.defaultCalendarId) return level.defaultCalendarId;
  const calendarId = await invoke<string>('create_academic_calendar', {
    calendar: {
      id: '', schoolId: level.schoolId, levelId: level.id,
      name: `${level.name} ${academicYearWindow().label}`,
      academicYearStart: level.academicYearStart, academicYearEnd: level.academicYearEnd,
      regionCode: level.regionCode || findSchoolRegion(state, level.schoolId),
      timezone: level.timezone || browserTimezone(), isDefault: true, createdAt: '', updatedAt: '',
    },
  });
  if (!calendarId) throw new Error('Failed to create academic calendar for this level.');
  level.defaultCalendarId = calendarId;
  return calendarId;
}

function findSchoolRegion(state: AppState, schoolId: string) {
  return state.schools.find((s) => s.id === schoolId)?.regionCode || 'local';
}

function classDateRange(klass: ClassView, level: Level) {
  return { start: klass.startDate || level.academicYearStart, end: klass.endDate || level.academicYearEnd };
}

function datesForWeekdays(startValue: string, endValue: string, weekdayValues: string[]) {
  const start = parseYmdUtc(startValue);
  const end = parseYmdUtc(endValue);
  if (!start || !end || end.getTime() < start.getTime()) return [];
  const allowedDays = new Set<number>(weekdayValues.map((w) => weekdayToJsDay(w)).filter((d) => d >= 0));
  const dates: string[] = [];
  const current = new Date(start.getTime());
  while (current.getTime() <= end.getTime()) {
    if (allowedDays.has(current.getUTCDay())) dates.push(formatYmdUtc(current));
    current.setUTCDate(current.getUTCDate() + 1);
  }
  return dates;
}
