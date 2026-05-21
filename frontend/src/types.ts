export type Page = 'overview' | 'classrooms' | 'syllabus' | 'lessonPlans' | 'calendar' | 'agents' | 'tests' | 'reports' | 'settings';
export type ClassTab = 'students' | 'calendar' | 'attendance' | 'tests';
export type CalendarMode = 'month' | 'week' | 'timeline';
export type ClassCalendarMode = 'month' | 'year';
export type LessonExportScope = 'lesson' | 'week' | 'month' | 'year';
export type LessonExportFormat = 'html' | 'md' | 'txt' | 'docx' | 'pdf';
export type LlmProvider = 'ollama' | 'opencode';

export type School = {
  id: string;
  name: string;
  logoPath?: string | null;
  regionCode?: string | null;
  timezone: string;
  academicYearLabel?: string | null;
};

export type Level = {
  id: string;
  schoolId: string;
  name: string;
  code?: string | null;
  academicYearStart: string;
  academicYearEnd: string;
  regionCode: string;
  timezone?: string | null;
  defaultCalendarId?: string | null;
  planningStatus: string;
};

export type ClassView = {
  id: string;
  schoolId: string;
  levelId: string;
  name: string;
  subjectName?: string | null;
  startDate?: string | null;
  endDate?: string | null;
  periodMinutes: number;
  schedulePatternSummary?: string | null;
  yearPlanId?: string | null;
};

export type SchoolDirectory = {
  school: School;
  levels: Array<Level & { classes: ClassView[] }>;
};

export type Student = {
  id: string;
  classId: string;
  fullName: string;
  preferredName?: string | null;
  studentCode?: string | null;
  age?: number | null;
  gender?: string | null;
  notesPrivate?: string | null;
};

export type SessionView = {
  id: string;
  sessionDate: string;
  title: string;
  completed: boolean;
};

export type AttendanceView = {
  id: string;
  sessionId: string;
  studentId: string;
  studentName: string;
  status: string;
  note?: string | null;
  recordedAt: string;
  updatedAt: string;
};

export type AgentInsight = {
  id: string;
  agentType: string;
  severity: string;
  title: string;
  body: string;
  createdAt: string;
};

export type ScheduleRule = {
  id: string;
  weekday: string;
  startTime?: string | null;
  endTime?: string | null;
  periodLabel?: string | null;
};

export type CurriculumUnit = {
  id: string;
  unitCode?: string | null;
  title: string;
  description?: string | null;
  estimatedLessons: number;
  estimatedWeeks: number;
  assessmentHint?: string | null;
  isRequired?: boolean;
  monthLabel?: string | null;
  scheduleSlotCount?: number;
};

export type SyllabusExtractionDiagnostics = {
  confidence: number;
  detectedLayout: string;
  unitCount: number;
  notes: string[];
  suggestedActions: string[];
};

export type YearPlanLesson = {
  id: string;
  teachingDate: string;
  weekday: string;
  lessonTitle: string;
  lessonObjective?: string | null;
  curriculumUnitId?: string | null;
  isBuffer: boolean;
  status: string;
  detailedPlanAttached: boolean;
  sequenceNumber: number;
};

export type CalendarClosureDay = {
  id: string;
  calendarId: string;
  closureDate: string;
  closureType: string;
  title?: string | null;
  sourceProvider?: string | null;
  createdAt: string;
};

export type YearPlanBundle = {
  yearPlan: {
    id: string;
    generationSummary?: string | null;
    totalTeachingDays: number;
    totalPlannedLessons: number;
  };
  lessons: YearPlanLesson[];
  warnings: string[];
};

export type LlmConfig = {
  provider: LlmProvider;
  model: string;
  available: boolean;
  detail: string;
};

export type LlmModelOption = {
  id: string;
  label: string;
};

export type GradingToolStatus = {
  key: string;
  label: string;
  available: boolean;
  detail: string;
};

export type GradingDiagnostics = {
  llmProvider: string;
  llmModel: string;
  llmAvailable: boolean;
  llmDetail: string;
  tools: GradingToolStatus[];
  storageRoot: string;
  storagePattern: string;
};

export type GradingToolInstallFailure = {
  key: string;
  label: string;
  detail: string;
};

export type GradingToolInstallResult = {
  installed: string[];
  alreadyAvailable: string[];
  failed: GradingToolInstallFailure[];
  notes: string[];
  postChecks: GradingToolStatus[];
};

export type TestTemplate = {
  id: string;
  classId: string;
  schoolId: string;
  levelId: string;
  title: string;
  instructions: string;
  scoringRubric: string;
  maxScore: number;
  status: string;
  createdAt: string;
  updatedAt: string;
};

export type TestDraft = {
  title: string;
  instructions: string;
  scoringRubric: string;
  maxScore: number;
};

export type TestSyllabusSection = {
  id: string;
  unitCode?: string | null;
  title: string;
  description?: string | null;
  estimatedLessons: number;
  assessmentHint?: string | null;
  isRequired: boolean;
};

export type StudentSubmission = {
  studentId: string;
  studentName: string;
  assessmentId: string;
  scoreValue: number | null;
  scoreLabel: string | null;
  teacherComment: string | null;
  attachmentId: string | null;
  fileName: string | null;
  mimeType: string | null;
  status: string;
};

export type StudentReportStatus = {
  studentId: string;
  studentName: string;
  reportId: string | null;
  reportExists: boolean;
  lastUpdated: string | null;
};

export type StudentReportView = {
  id: string;
  studentId: string;
  classId: string;
  reportText: string;
  teacherAdviceText: string | null;
};

export type TeacherProfile = {
  name: string;
  title: string;
  image?: string | null;
};

export type AppState = {
  page: Page;
  classTab: ClassTab;
  calendarMode: CalendarMode;
  classCalendarMode: ClassCalendarMode;
  classCalendarMonth: string;
  showAttendanceHistory: boolean;
  backend: boolean;
  loading: boolean;
  message: string;
  schools: School[];
  levels: Level[];
  classes: ClassView[];
  students: Student[];
  sessions: SessionView[];
  attendanceRecords: AttendanceView[];
  scheduleRules: ScheduleRule[];
  insights: AgentInsight[];
  extractedUnits: CurriculumUnit[];
  syllabusExtractionDiagnostics: SyllabusExtractionDiagnostics | null;
  generatedLessons: YearPlanLesson[];
  currentSyllabusId: string;
  lessonExportScope: LessonExportScope;
  lessonExportFormat: LessonExportFormat;
  lessonTemplate: string;
  lessonTemplateName: string;
  lessonContextNotes: string;
  calendarClosures: CalendarClosureDay[];
  planMessage: string;
  syllabusBusy: boolean;
  lessonPlansBusy: boolean;
  detailedPlanProgressActive: boolean;
  detailedPlanProgressTotal: number;
  detailedPlanProgressCompleted: number;
  detailedPlanProgressMessage: string;
  setupNotice: string;
  setupBusy: string;
  directory: SchoolDirectory[];
  selectedSchoolId: string;
  selectedLevelId: string;
  selectedClassId: string;
  llmConfig: LlmConfig | null;
  llmModels: LlmModelOption[];
  gradingDiagnostics: GradingDiagnostics | null;
  gradingToolsInstallBusy: boolean;
  gradingToolInstallNotes: Record<string, string>;
  teacherProfile: TeacherProfile;
  tests: TestTemplate[];
  currentTestId: string;
  testSubmissions: StudentSubmission[];
  classroomTestSubmissions: Record<string, StudentSubmission[]>;
  classroomMatrixStatus: Record<string, 'idle' | 'uploaded' | 'grading' | 'graded' | 'failed'>;
  classroomMatrixLastGradedAt: Record<string, string>;
  classroomMatrixGradeAllBusy: boolean;
  classroomMatrixCompactMode: boolean;
  reportStatuses: StudentReportStatus[];
  reportInstructions: string;
  reportWordCount: number;
  allCalendarLessons: CalendarLessonView[];
  allCalendarSessions: CalendarClassSessionView[];
};

export type CalendarLessonView = {
  lesson: YearPlanLesson;
  classId: string;
  className: string;
  levelName: string;
  schoolName: string;
  subjectName: string | null;
};

export type CalendarClassSessionView = {
  session: SessionView;
  className: string;
  levelName: string;
  schoolName: string;
  subjectName: string | null;
};

export type LessonPlanJson = {
  v: string;
  objective: string;
  transferGoal: string;
  enduringUnderstanding: string;
  essentialQuestions: string[];
  knowledge: string[];
  skills: string[];
  successCriteria: string[];
  assessment: string;
  performanceTask?: string;
  materials: string[];
  vocabulary: string[];
  lessonFlow: Array<{ phase: string; time: string; description: string }>;
  differentiation: string;
  homework?: string;
  crossCurricular?: string;
  backwardDesign?: Record<string, unknown>;
};

declare global {
  interface Window {
    __TAURI__?: {
      core?: {
        invoke: <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
      };
      invoke?: <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
    };
  }
}
