import './style.css';
import './animations';
import 'flag-icons/css/flag-icons.min.css';
import type {
  Page, ClassTab, CalendarMode, ClassCalendarMode, LessonExportScope, LessonExportFormat,
  School, ClassView, Student, AgentInsight, ScheduleRule, CurriculumUnit,
  LlmConfig, GradingDiagnostics, GradingToolInstallResult, TestTemplate, TestDraft, TestSyllabusSection, StudentSubmission, StudentReportStatus, SyllabusExtractionDiagnostics,
  StudentReportView, YearPlanBundle, YearPlanLesson, CalendarClassSessionView,
} from './types';
import { state } from './state';
import {
  invoke, loadSchoolDirectory, loadHierarchy, refreshSelectedHierarchy, loadClassData,
  ensureAttendanceSessionsForSelectedClass, syncPublicHolidaysForLevel,
  ensureCalendarForLevel, loadSettings, loadModels, loadSchoolImages,
  loadAttendanceData, loadAllCalendarLessons, loadAllCalendarSessions,
  selectedSchool, selectedLevel, selectedClass,
  saveTeacherProfile,
} from './state';
import {
  icon, escapeHtml, emptyState, capitalize, getGreeting, pageTitle,
  parseYmdUtc, formatYmdUtc, shortDate, formatDisplayDate, isTodayIso, currentMonthKey,
  countryName, countryCodes, academicYearWindow, browserTimezone, academicYearOptions,
  weekdays, weekLabels, downloadTextFile, readImageAsDataUrl, showToast,
  classDateRange, monthGridDates, weekKey, parseLessonPlan, downloadExport,
} from './utils';

const app = document.querySelector<HTMLDivElement>('#app');

if (!app) {
  throw new Error('Missing #app root');
}

const root = app;

const pages: Array<{ id: Page; label: string; icon: string }> = [
  { id: 'overview', label: 'Overview', icon: 'layout' },
  { id: 'syllabus', label: 'Syllabus', icon: 'book' },
  { id: 'lessonPlans', label: 'Lesson Plans', icon: 'book' },
  { id: 'classrooms', label: 'Classrooms', icon: 'users' },
  { id: 'calendar', label: 'Calendar', icon: 'calendar' },
  { id: 'agents', label: 'Agents', icon: 'cpu' },
  { id: 'tests', label: 'Tests', icon: 'check-circle' },
  { id: 'reports', label: 'Reports', icon: 'book' },
  { id: 'settings', label: 'Settings', icon: 'settings' },
];

const agentCards = [
  { type: 'attendance', title: 'Attendance Risk', body: 'Identifies students whose attendance patterns need review.', color: 'peach', icon: 'users' },
  { type: 'planning', title: 'Lesson Pacing', body: 'Detects when planned lessons are falling behind the calendar.', color: 'violet', icon: 'calendar' },
  { type: 'performance', title: 'Academic Performance', body: 'Monitors assessment score trends and flags declining performance.', color: 'rose', icon: 'cpu' },
  { type: 'assignments', title: 'Assignment Completion', body: 'Flags completed sessions with missing attendance or assessment records.', color: 'mint', icon: 'check-circle' },
  { type: 'engagement', title: 'Engagement Pulse', body: 'Looks for changes in session completion and late arrival patterns.', color: 'sky', icon: 'layout' },
  { type: 'reports', title: 'Report Writer', body: 'Drafts student report language and tracks which reports need updating.', color: 'lavender', icon: 'book' },
  { type: 'syllabus', title: 'Syllabus Extraction', body: 'Verifies extracted units match the syllabus text and flags issues.', color: 'amber', icon: 'upload' },
];

type PersistedSyllabusDraft = {
  currentSyllabusId: string;
  planMessage: string;
  extractedUnits: CurriculumUnit[];
  savedAt: string;
};

type PersistedSyllabusState = {
  version: 1;
  lastContext: {
    selectedSchoolId: string;
    selectedLevelId: string;
    selectedClassId: string;
  };
  drafts: Record<string, PersistedSyllabusDraft>;
};

const SYLLABUS_STATE_STORAGE_KEY = 'edutrack.syllabusPlanning.v1';
const REQUIRED_STARTUP_TOOL_KEYS = ['opencode', 'tesseract', 'whisper', 'pandoc'] as const;

type StartupToolStatus = 'pending' | 'installing' | 'available' | 'failed';

type StartupInstallTool = {
  key: string;
  label: string;
  status: StartupToolStatus;
  progress: number;
  detail: string;
};

type StartupGatePhase = 'checking' | 'installing' | 'verifying' | 'failed' | 'ready';

const startupInstallGate: {
  active: boolean;
  busy: boolean;
  phase: StartupGatePhase;
  message: string;
  tools: StartupInstallTool[];
} = {
  active: false,
  busy: false,
  phase: 'checking',
  message: 'Checking required tools...',
  tools: REQUIRED_STARTUP_TOOL_KEYS.map((key) => ({
    key,
    label: startupToolLabel(key),
    status: 'pending',
    progress: 6,
    detail: 'Waiting to check availability.',
  })),
};

const lessonExportSelection = new Set<string>();
const sortedCountryChoices = countryCodes
  .map((code) => ({ code, name: countryName(code) ?? code }))
  .sort((a, b) => a.name.localeCompare(b.name));
const monthAliasToFull: Record<string, string> = {
  january: 'january', jan: 'january',
  february: 'february', feb: 'february',
  march: 'march', mar: 'march',
  april: 'april', apr: 'april',
  may: 'may',
  june: 'june', jun: 'june',
  july: 'july', jul: 'july',
  august: 'august', aug: 'august',
  september: 'september', sep: 'september', sept: 'september',
  october: 'october', oct: 'october',
  november: 'november', nov: 'november',
  december: 'december', dec: 'december',
};
const monthAliasRegex = /\b(january|jan|february|feb|march|mar|april|apr|may|june|jun|july|jul|august|aug|september|sep|sept|october|oct|november|nov|december|dec)\b/g;
let activeCountryPickerClose: (() => void) | null = null;
let countryPickerOutsideBound = false;
let detailedPlanCancelRequested = false;

type AttachDetailedPlansResult = {
  attachedCount: number;
  alreadyAttachedCount: number;
  unmatchedCount: number;
  totalDetailedCount: number;
};

type DeleteSchoolDataResult = {
  schoolsDeleted: number;
  levelsDeleted: number;
  classesDeleted: number;
  studentsDeleted: number;
  yearPlansDeleted: number;
  yearPlanLessonsDeleted: number;
};

function ensureCountryPickerOutsideBinding() {
  if (countryPickerOutsideBound) return;
  document.addEventListener('pointerdown', (event) => {
    const activePicker = document.querySelector<HTMLElement>('[data-country-picker][data-country-open="1"]');
    if (!activePicker || !activeCountryPickerClose) return;
    const target = event.target as Node | null;
    if (target && activePicker.contains(target)) return;
    activeCountryPickerClose();
  }, true);
  countryPickerOutsideBound = true;
}

function countryPickerMarkup() {
  return `
    <div class="country-picker" data-country-picker>
      <input id="school-region" type="hidden" value="" />
      <button type="button" class="country-picker-trigger" data-country-trigger aria-expanded="false" aria-haspopup="listbox" aria-controls="country-picker-options">
        <span class="fi fi-xx country-flag" data-country-selected-flag aria-hidden="true"></span>
        <span data-country-selected-label>Select country</span>
      </button>
      <div class="country-picker-popover" data-country-popover hidden>
        <input type="text" class="country-picker-search" data-country-filter placeholder="Search country" />
        <div id="country-picker-options" class="country-picker-options" role="listbox" aria-label="Countries">
          ${sortedCountryChoices.map((country) => `
            <button type="button" class="country-picker-option" role="option" aria-selected="false" data-country-option="${country.code}" data-country-name="${escapeHtml(country.name.toLowerCase())}">
              <span class="fi fi-${country.code.toLowerCase()} country-flag" aria-hidden="true"></span>
              <span>${escapeHtml(country.name)}</span>
            </button>
          `).join('')}
        </div>
      </div>
    </div>
  `;
}

function nonBufferLessons() {
  return sortedGeneratedLessons().filter((lesson) => !lesson.isBuffer);
}

function detailedPlanProgressPercent() {
  if (state.detailedPlanProgressTotal <= 0) return 0;
  return Math.round((state.detailedPlanProgressCompleted / state.detailedPlanProgressTotal) * 100);
}

function updateDetailedPlanProgressOverlay() {
  const overlay = document.querySelector<HTMLElement>('[data-detailed-plan-progress-overlay]');
  if (!overlay) {
    render();
    return;
  }
  const percent = detailedPlanProgressPercent();
  const count = overlay.querySelector<HTMLElement>('[data-detailed-plan-progress-count]');
  const message = overlay.querySelector<HTMLElement>('[data-detailed-plan-progress-message]');
  const percentLabel = overlay.querySelector<HTMLElement>('[data-detailed-plan-progress-percent]');
  const bar = overlay.querySelector<HTMLElement>('[data-detailed-plan-progress-bar]');
  if (count) count.textContent = `Progress: ${state.detailedPlanProgressCompleted} / ${state.detailedPlanProgressTotal} lessons completed`;
  if (message) message.textContent = `Current task: ${state.detailedPlanProgressMessage || 'Preparing generation'}`;
  if (percentLabel) percentLabel.textContent = `Completed ${percent}%`;
  if (bar) bar.style.width = `${percent}%`;
}

function waitForPaint() {
  return new Promise<void>((resolve) => {
    if (typeof window.requestAnimationFrame === 'function') {
      window.requestAnimationFrame(() => resolve());
      return;
    }
    setTimeout(() => resolve(), 0);
  });
}

async function renderDetailedPlanProgressTick() {
  updateDetailedPlanProgressOverlay();
  await waitForPaint();
}

function sleepMs(ms: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function compactProviderError(message: string) {
  const noFragments = message.replace(/\[\?[0-9;]*[A-Za-z]|\[[0-9;]*[A-Za-z]|\[K/g, ' ');
  const compact = noFragments.replace(/\s+/g, ' ').trim();
  const lower = compact.toLowerCase();
  const idx429 = lower.indexOf('error: 429');
  if (idx429 >= 0) return compact.slice(idx429);
  const idxTooMany = lower.indexOf('429 too many requests');
  if (idxTooMany >= 0) return compact.slice(idxTooMany);
  return compact;
}

function isCloudBackedModel(modelId: string | undefined | null) {
  if (!modelId) return false;
  const lower = modelId.toLowerCase();
  return lower.includes(':cloud') || lower.includes('-cloud');
}

function startDetailedPlanHeartbeat(labelFactory: () => string) {
  const startedAt = Date.now();
  let tick = 0;
  const timer = window.setInterval(() => {
    tick += 1;
    const elapsedSeconds = Math.floor((Date.now() - startedAt) / 1000);
    const dots = '.'.repeat(tick % 4);
    state.detailedPlanProgressMessage = `${labelFactory()}${dots} (${elapsedSeconds}s)`;
    updateDetailedPlanProgressOverlay();
  }, 300);
  return () => window.clearInterval(timer);
}

function unitTimelineFromText(unit: CurriculumUnit) {
  const explicitMonthLabel = (unit.monthLabel ?? '').trim();
  if (explicitMonthLabel) return explicitMonthLabel;

  const source = `${unit.title} ${unit.description ?? ''}`.toLowerCase();
  const assignedTimelineMatch = source.match(/assigned\s+timeline\s*:\s*([^\n.]+)/i);
  if (assignedTimelineMatch?.[1]?.trim()) {
    return assignedTimelineMatch[1].trim();
  }

  const foundMonths: string[] = [];
  for (const match of source.matchAll(monthAliasRegex)) {
    const token = match[1]?.toLowerCase();
    if (!token) continue;
    const full = monthAliasToFull[token];
    if (full) foundMonths.push(full);
  }

  const compactDateRange = source.match(/\b\d{1,2}[/-]\d{1,2}[/-]\d{2,4}\s*(?:to|-)\s*\d{1,2}[/-]\d{1,2}[/-]\d{2,4}\b/);
  if (compactDateRange) return compactDateRange[0];
  const isoDateRange = source.match(/\b\d{4}-\d{2}-\d{2}\s*(?:to|-)\s*\d{4}-\d{2}-\d{2}\b/);
  if (isoDateRange) return isoDateRange[0];
  if (foundMonths.length >= 2) {
    const unique = Array.from(new Set(foundMonths));
    return `${capitalize(unique[0])} to ${capitalize(unique[unique.length - 1])}`;
  }
  if (foundMonths.length === 1) return capitalize(foundMonths[0]);
  return null;
}

function unitAssignedRangeLabel(unit: CurriculumUnit) {
  const scheduled = nonBufferLessons().filter((lesson) => lesson.curriculumUnitId === unit.id);
  if (scheduled.length > 0) {
    const sorted = [...scheduled].sort((a, b) => a.teachingDate.localeCompare(b.teachingDate));
    const first = sorted[0];
    const last = sorted[sorted.length - 1];
    if (first.teachingDate === last.teachingDate) return formatDisplayDate(first.teachingDate);
    return `${formatDisplayDate(first.teachingDate)} to ${formatDisplayDate(last.teachingDate)}`;
  }
  return unitTimelineFromText(unit) ?? 'Timeline not detected in source';
}

function syllabusContextKey(
  schoolId = state.selectedSchoolId,
  levelId = state.selectedLevelId,
  classId = state.selectedClassId,
) {
  if (classId) return `class:${classId}`;
  if (levelId) return `level:${levelId}`;
  if (schoolId) return `school:${schoolId}`;
  return 'global';
}

function normalizeStoredUnits(raw: unknown): CurriculumUnit[] {
  if (!Array.isArray(raw)) return [];
  return raw.reduce<CurriculumUnit[]>((acc, item, index) => {
    if (!item || typeof item !== 'object') return acc;
    const unit = item as Partial<CurriculumUnit>;
    if (typeof unit.title !== 'string' || !unit.title.trim()) return acc;
    const estimatedLessons = Number(unit.estimatedLessons);
    const estimatedWeeks = Number(unit.estimatedWeeks);
    acc.push({
      id: typeof unit.id === 'string' && unit.id ? unit.id : `draft-unit-${index + 1}`,
      unitCode: typeof unit.unitCode === 'string' ? unit.unitCode : null,
      title: unit.title.trim(),
      description: typeof unit.description === 'string' ? unit.description : null,
      estimatedLessons: Number.isFinite(estimatedLessons) ? estimatedLessons : 4,
      estimatedWeeks: Number.isFinite(estimatedWeeks) ? estimatedWeeks : 4,
      assessmentHint: typeof unit.assessmentHint === 'string' ? unit.assessmentHint : null,
      isRequired: typeof unit.isRequired === 'boolean' ? unit.isRequired : true,
      monthLabel: typeof unit.monthLabel === 'string' && unit.monthLabel.trim() ? unit.monthLabel.trim() : null,
      scheduleSlotCount: Number.isFinite(Number(unit.scheduleSlotCount)) ? Number(unit.scheduleSlotCount) : 0,
    });
    return acc;
  }, []);
}

function readPersistedSyllabusState(): PersistedSyllabusState | null {
  try {
    const raw = localStorage.getItem(SYLLABUS_STATE_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<PersistedSyllabusState>;
    const drafts = parsed.drafts && typeof parsed.drafts === 'object' ? parsed.drafts : {};
    const normalizedDrafts: Record<string, PersistedSyllabusDraft> = {};
    for (const [key, value] of Object.entries(drafts)) {
      if (!value || typeof value !== 'object') continue;
      const draft = value as Partial<PersistedSyllabusDraft>;
      normalizedDrafts[key] = {
        currentSyllabusId: typeof draft.currentSyllabusId === 'string' ? draft.currentSyllabusId : '',
        planMessage: typeof draft.planMessage === 'string' ? draft.planMessage : '',
        extractedUnits: normalizeStoredUnits(draft.extractedUnits),
        savedAt: typeof draft.savedAt === 'string' ? draft.savedAt : new Date().toISOString(),
      };
    }
    const last = parsed.lastContext ?? { selectedSchoolId: '', selectedLevelId: '', selectedClassId: '' };
    return {
      version: 1,
      lastContext: {
        selectedSchoolId: typeof last.selectedSchoolId === 'string' ? last.selectedSchoolId : '',
        selectedLevelId: typeof last.selectedLevelId === 'string' ? last.selectedLevelId : '',
        selectedClassId: typeof last.selectedClassId === 'string' ? last.selectedClassId : '',
      },
      drafts: normalizedDrafts,
    };
  } catch {
    return null;
  }
}

function writePersistedSyllabusState(payload: PersistedSyllabusState) {
  try {
    localStorage.setItem(SYLLABUS_STATE_STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // Ignore storage failures.
  }
}

function persistSyllabusPlanningState() {
  const existing = readPersistedSyllabusState() ?? {
    version: 1 as const,
    lastContext: { selectedSchoolId: '', selectedLevelId: '', selectedClassId: '' },
    drafts: {},
  };
  existing.lastContext = {
    selectedSchoolId: state.selectedSchoolId,
    selectedLevelId: state.selectedLevelId,
    selectedClassId: state.selectedClassId,
  };
  const key = syllabusContextKey();
  if (state.currentSyllabusId || state.planMessage || state.extractedUnits.length > 0) {
    existing.drafts[key] = {
      currentSyllabusId: state.currentSyllabusId,
      planMessage: state.planMessage,
      extractedUnits: state.extractedUnits,
      savedAt: new Date().toISOString(),
    };
  } else {
    delete existing.drafts[key];
  }
  writePersistedSyllabusState(existing);
}

function applySyllabusPlanningStateForCurrentContext(clearIfMissing = true) {
  const existing = readPersistedSyllabusState();
  const draft = existing?.drafts[syllabusContextKey()];
  if (!draft) {
    if (clearIfMissing) {
      state.currentSyllabusId = '';
      state.planMessage = '';
      state.extractedUnits = [];
      state.syllabusExtractionDiagnostics = null;
    }
    return;
  }
  state.currentSyllabusId = draft.currentSyllabusId;
  state.planMessage = draft.planMessage;
  state.extractedUnits = draft.extractedUnits;
  state.syllabusExtractionDiagnostics = null;
}

function startupToolLabel(key: string) {
  switch (key) {
    case 'opencode': return 'OpenCode CLI';
    case 'tesseract': return 'Tesseract OCR';
    case 'whisper': return 'Whisper CLI';
    case 'pandoc': return 'Pandoc';
    default: return key;
  }
}

function truncateText(value: string, maxChars: number) {
  if (value.length <= maxChars) return value;
  return `${value.slice(0, Math.max(0, maxChars - 1)).trimEnd()}...`;
}

function summarizeStartupToolDetail(raw: string, key?: string) {
  const normalized = raw
    .replace(/\u001b\[[0-9;]*[A-Za-z]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  if (!normalized) return 'No additional details provided.';
  const lower = normalized.toLowerCase();
  if (lower.includes('unicodeencodeerror') && lower.includes('cp1252')) {
    return 'Whisper CLI is installed, but the help output cannot render in the current Windows code page. Transcription can still work for normal runs.';
  }
  if (key === 'pandoc' && lower.includes('not found in path')) {
    return 'Pandoc is missing from PATH. Install it or restart EduTrack after installation so PATH changes are visible.';
  }
  if (key === 'tesseract' && lower.includes('not found in path')) {
    return 'Tesseract is missing from PATH. Install it or restart EduTrack after installation so PATH changes are visible.';
  }
  if (lower.includes('traceback')) {
    return truncateText(normalized, 220);
  }
  return truncateText(normalized, 220);
}

function startupToolProgressByStatus(status: StartupToolStatus) {
  switch (status) {
    case 'available': return 100;
    case 'installing': return 64;
    case 'failed': return 100;
    default: return 12;
  }
}

function startupGateProgress() {
  const total = startupInstallGate.tools.length || 1;
  const completed = startupInstallGate.tools.filter((tool) => tool.status === 'available').length;
  return {
    total,
    completed,
    percent: Math.round((completed / total) * 100),
  };
}

function syncStartupGateFromDiagnostics(diagnostics: GradingDiagnostics | null) {
  for (const tool of startupInstallGate.tools) {
    const diagnostic = diagnostics?.tools.find((entry) => entry.key === tool.key);
    if (!diagnostic) {
      if (tool.status !== 'failed') {
        tool.status = 'pending';
        tool.progress = startupToolProgressByStatus('pending');
        tool.detail = 'Not verified yet.';
      }
      continue;
    }
    if (diagnostic.available) {
      tool.status = 'available';
      tool.progress = startupToolProgressByStatus('available');
      tool.detail = summarizeStartupToolDetail(diagnostic.detail, tool.key);
      continue;
    }
    if (tool.status !== 'installing' && tool.status !== 'failed') {
      tool.status = 'pending';
      tool.progress = startupToolProgressByStatus('pending');
      tool.detail = summarizeStartupToolDetail(diagnostic.detail, tool.key);
    }
  }
}

function startupGateStatusLabel(status: StartupToolStatus) {
  switch (status) {
    case 'available': return 'AVAILABLE';
    case 'installing': return 'INSTALLING';
    case 'failed': return 'FAILED';
    default: return 'PENDING';
  }
}

function renderStartupInstallGate() {
  const { total, completed, percent } = startupGateProgress();
  const canRetry = startupInstallGate.phase === 'failed' && !startupInstallGate.busy;
  return `
    <section class="startup-gate">
      <div class="startup-gate-layout">
        <article class="startup-gate-card">
          <p class="eyebrow">Startup Checks</p>
          <h2>Preparing EduTrack</h2>
          <p class="startup-gate-message">${escapeHtml(startupInstallGate.message)}</p>
          <div class="startup-gate-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${percent}">
            <span class="startup-gate-progress-fill" style="width:${percent}%"></span>
          </div>
          <p class="startup-gate-meta">${completed} of ${total} required tools ready (${percent}%)</p>
          <div class="startup-gate-tools">
            ${startupInstallGate.tools.map((tool) => `
              <article class="startup-tool-row">
                <div class="startup-tool-head">
                  <strong>${escapeHtml(tool.label)}</strong>
                  <span class="startup-tool-status ${tool.status}">${startupGateStatusLabel(tool.status)}</span>
                </div>
                <div class="startup-tool-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${tool.progress}">
                  <span class="startup-tool-progress-fill ${tool.status}" style="width:${tool.progress}%"></span>
                </div>
                <p class="startup-tool-detail">${escapeHtml(tool.detail)}</p>
              </article>
            `).join('')}
          </div>
          ${startupInstallGate.phase === 'failed' ? '<p class="startup-gate-failure">Install failed for one or more tools. You can retry now or continue and install missing tools later from Settings.</p>' : ''}
          <div class="startup-gate-actions">
            <button class="primary-button" type="button" data-startup-install-retry ${canRetry ? '' : 'disabled'}>${icon('upload')} Retry install</button>
            ${canRetry ? '<button class="ghost-button" type="button" data-startup-continue-limited>Continue with limited features</button>' : ''}
          </div>
        </article>
        <aside class="startup-gate-brand" aria-hidden="true">
          <img src="/logo.png" alt="" class="startup-gate-logo" />
        </aside>
      </div>
    </section>
  `;
}

function bindStartupInstallGateEvents() {
  document.querySelector<HTMLButtonElement>('[data-startup-install-retry]')?.addEventListener('click', () => {
    void runStartupInstallGate();
  });
  document.querySelector<HTMLButtonElement>('[data-startup-continue-limited]')?.addEventListener('click', () => {
    startupInstallGate.active = false;
    startupInstallGate.busy = false;
    render();
  });
}

async function runStartupInstallGate() {
  if (!state.backend || startupInstallGate.busy) return;

  startupInstallGate.active = true;
  startupInstallGate.busy = true;
  startupInstallGate.phase = 'checking';
  startupInstallGate.message = 'Checking required tools...';
  for (const tool of startupInstallGate.tools) {
    if (tool.status !== 'available') {
      tool.status = 'pending';
      tool.progress = startupToolProgressByStatus('pending');
      tool.detail = 'Waiting to check availability.';
    }
  }
  render();
  await waitForPaint();

  await refreshGradingDiagnostics();
  syncStartupGateFromDiagnostics(state.gradingDiagnostics);
  render();
  await waitForPaint();

  const requiredKeys = [...REQUIRED_STARTUP_TOOL_KEYS];
  for (const key of requiredKeys) {
    const target = startupInstallGate.tools.find((tool) => tool.key === key);
    if (!target || target.status === 'available') continue;

    startupInstallGate.phase = 'installing';
    target.status = 'installing';
    target.progress = startupToolProgressByStatus('installing');
    target.detail = `Installing ${target.label}...`;
    startupInstallGate.message = `Installing ${target.label}...`;
    render();
    await waitForPaint();

    try {
      const result = await invoke<GradingToolInstallResult>('install_grading_tools', { toolKeys: [key] });
      if (result?.installed?.includes(key)) {
        state.gradingToolInstallNotes[key] = 'Installed successfully.';
      } else if (result?.alreadyAvailable?.includes(key)) {
        state.gradingToolInstallNotes[key] = 'Already available.';
      } else {
        const failed = result?.failed?.find((item) => item.key === key);
        const note = (result?.notes ?? []).find((line) => line.toLowerCase().includes(key));
        const detail = summarizeStartupToolDetail(note || failed?.detail || 'Install attempt failed.', key);
        target.status = 'failed';
        target.progress = startupToolProgressByStatus('failed');
        target.detail = detail;
        state.gradingToolInstallNotes[key] = detail;
      }
    } catch (error) {
      const detail = summarizeStartupToolDetail(error instanceof Error ? error.message : String(error), key);
      target.status = 'failed';
      target.progress = startupToolProgressByStatus('failed');
      target.detail = detail;
      state.gradingToolInstallNotes[key] = detail;
    }

    startupInstallGate.phase = 'verifying';
    target.progress = Math.max(target.progress, 82);
    if (target.status !== 'failed') {
      target.detail = `Verifying ${target.label} installation...`;
    }
    startupInstallGate.message = `Verifying ${target.label}...`;
    await refreshGradingDiagnostics();
    syncStartupGateFromDiagnostics(state.gradingDiagnostics);
    const verified = startupInstallGate.tools.find((tool) => tool.key === key);
    if (verified && verified.status !== 'available' && verified.status !== 'failed') {
      verified.status = 'failed';
      verified.progress = startupToolProgressByStatus('failed');
      verified.detail = summarizeStartupToolDetail(
        verified.detail || `${verified.label} is still missing after install attempt.`,
        verified.key,
      );
      state.gradingToolInstallNotes[key] = verified.detail;
    }
    render();
    await waitForPaint();
  }

  await refreshGradingDiagnostics();
  syncStartupGateFromDiagnostics(state.gradingDiagnostics);
  const unresolved = startupInstallGate.tools.filter((tool) => tool.status !== 'available');
  if (unresolved.length === 0) {
    startupInstallGate.phase = 'ready';
    startupInstallGate.message = 'All required tools are installed.';
    startupInstallGate.active = false;
    startupInstallGate.busy = false;
    render();
    return;
  }

  startupInstallGate.phase = 'failed';
  startupInstallGate.message = `${unresolved.length} required tool${unresolved.length === 1 ? '' : 's'} still unavailable.`;
  startupInstallGate.busy = false;
  startupInstallGate.active = true;
  render();
}

async function loadInitialData() {
  state.backend = Boolean(invoke<unknown>('initialize_db'));
  if (!state.backend) {
    state.loading = false;
    state.message = 'Open the EduTrack desktop app to load local data.';
    render();
    return;
  }

  try {
    const schools = await invoke<School[]>('get_schools');
    state.schools = schools ?? [];
    const persisted = readPersistedSyllabusState();
    const preferredSchoolId = persisted?.lastContext.selectedSchoolId ?? '';
    state.selectedSchoolId = state.schools.some((school) => school.id === preferredSchoolId)
      ? preferredSchoolId
      : state.schools[0]?.id ?? '';
    await loadHierarchy(state);
    if (persisted?.lastContext.selectedLevelId || persisted?.lastContext.selectedClassId) {
      await refreshSelectedHierarchy(
        state,
        state.selectedSchoolId,
        persisted.lastContext.selectedLevelId || undefined,
        persisted.lastContext.selectedClassId || undefined,
      );
    }
    applySyllabusPlanningStateForCurrentContext();
    await loadSchoolDirectory(state);
    await loadSchoolImages(state);
    await loadSettings(state);
    const profileJson = await invoke<string | null>('get_teacher_profile');
    if (profileJson) {
      try {
        const parsed = JSON.parse(profileJson) as Partial<typeof state.teacherProfile>;
        if (parsed.name) {
          state.teacherProfile = {
            name: parsed.name?.trim() || 'Teacher',
            title: parsed.title?.trim() || 'Teacher workspace',
            image: parsed.image || null,
          };
          if (state.teacherProfile.image && !state.teacherProfile.image.startsWith('data:')) {
            try { state.teacherProfile.image = await invoke<string>('read_image_file', { path: state.teacherProfile.image }); }
            catch { state.teacherProfile.image = null; }
          }
          saveTeacherProfile(state.teacherProfile);
        }
      } catch { /* ignore corrupt profile */ }
    }
    state.message = '';
  } catch (error) {
    state.message = String(error);
  } finally {
    state.loading = false;
    if (state.backend) {
      startupInstallGate.active = false;
      startupInstallGate.phase = 'checking';
      startupInstallGate.message = 'Checking required tools...';
      for (const tool of startupInstallGate.tools) {
        if (tool.status !== 'available') {
          tool.status = 'pending';
          tool.progress = startupToolProgressByStatus('pending');
          tool.detail = 'Waiting to check availability.';
        }
      }
      await refreshGradingDiagnostics();
      syncStartupGateFromDiagnostics(state.gradingDiagnostics);
    }
    render();
  }
}

function shell(content: string) {
  const activeSchool = selectedSchool(state);
  const activeLevel = selectedLevel(state);
  const activeClass = selectedClass(state);
  const backendNotice = state.backend
    ? ''
    : `<div class="notice notice-warn animate-fade-in-up">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="20" height="20"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        <span>Desktop app required to load local data.</span>
      </div>`;

  root.innerHTML = `
    <a href="#main-content" class="skip-link">Skip to main content</a>
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark"><img src="/logo.png" alt="EduTrack" style="width:28px;height:28px" /></div>
        <div>
          <strong>EduTrack</strong>
          <span>Teacher workspace</span>
        </div>
      </div>
      <nav class="side-nav">
        ${pages.map((page, index) => `
          <button class="nav-item ${state.page === page.id ? 'active' : ''} animate-fade-in-up" style="animation-delay: ${index * 50}ms" data-page="${page.id}" type="button">
            ${icon(page.icon)}
            <span>${page.label}</span>
          </button>
        `).join('')}
      </nav>
      <div class="teacher-card animate-fade-in-up delay-300">
        ${teacherAvatar('avatar')}
        <div>
          <strong>${escapeHtml(state.teacherProfile.name)}</strong>
          <span>${escapeHtml(state.teacherProfile.title || (state.backend ? 'Connected' : 'Offline'))}</span>
        </div>
      </div>
    </aside>
    <main class="workspace" id="main-content">
      <header class="topbar animate-fade-in-up">
        <div>
          <p class="eyebrow">${getGreeting()}</p>
          <h1>${pageTitle(state.page)}</h1>
        </div>
        <div class="top-actions">
          <button type="button" class="ghost-button">${icon('check-circle')} Review day</button>
        </div>
      </header>
      ${state.page === 'overview' ? renderSetupWorkflow() : ''}
      <section class="context-strip animate-fade-in-up delay-100">
        ${contextChip('School', activeSchool?.name ?? 'No school', 'selectedSchoolId', state.schools)}
        ${contextChip('Level', activeLevel?.name ?? 'No level', 'selectedLevelId', state.levels)}
        ${contextChip('Class', activeClass?.name ?? 'No class', 'selectedClassId', state.classes)}
      </section>
      ${pulseRail()}
      ${backendNotice}
      ${state.message && state.backend ? `
        <div class="notice notice-warn animate-fade-in-up">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="20" height="20"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          <span>${state.message}</span>
        </div>
      ` : ''}
      ${state.insights.length > 0 && state.selectedClassId ? agentSignalBar() : ''}
      ${content}
    </main>
    <div id="toast-container" aria-live="polite"></div>
  `;

  bindShellEvents();
}

function pulseRail() {
  const connected = state.backend ? 'Connected' : 'Offline';
  const roster = selectedClass(state) ? `${state.students.length} students` : 'No class';
  const sessions = state.sessions.length > 0 ? `${state.sessions.length} sessions` : 'No sessions';
  const alerts = state.insights.length > 0 ? `${state.insights.length} insight${state.insights.length === 1 ? '' : 's'}` : 'Quiet';
  return `
    <section class="pulse-rail animate-fade-in-up delay-200" aria-label="Workspace pulse">
      ${pulseItem('Workspace', connected, state.backend ? 'ok' : 'warn')}
      ${pulseItem('Roster', roster, state.students.length > 0 ? 'ok' : 'idle')}
      ${pulseItem('Schedule', sessions, state.sessions.length > 0 ? 'ok' : 'idle')}
      ${pulseItem('Agents', alerts, state.insights.length > 0 ? 'warn' : 'idle')}
    </section>
  `;
}

function pulseItem(label: string, value: string, tone: 'ok' | 'warn' | 'idle') {
  return `<div class="pulse-item ${tone}"><span class="pulse-dot"></span><span>${label}</span><strong>${escapeHtml(value)}</strong></div>`;
}

function agentSignalBar() {
  const severityOrder = ['high', 'warning', 'neutral'];
  const sorted = [...state.insights].sort((a, b) => severityOrder.indexOf(a.severity) - severityOrder.indexOf(b.severity));
  const top = sorted.slice(0, 3);
  const highCount = state.insights.filter((i) => i.severity === 'high').length;
  const warnCount = state.insights.filter((i) => i.severity === 'warning').length;
  return `
    <section class="agent-signal-bar animate-fade-in-up" aria-label="Agent signals">
      <span class="agent-signal-icon">${icon(highCount > 0 ? 'alert-circle' : 'check-circle')}</span>
      <div class="agent-signal-summary">
        <strong>${highCount + warnCount} signal${highCount + warnCount === 1 ? '' : 's'}</strong>
        ${highCount > 0 ? `<span class="signal-high">${highCount} high</span>` : ''}
        ${warnCount > 0 ? `<span class="signal-warn">${warnCount} warnings</span>` : ''}
      </div>
      <div class="agent-signal-items">
        ${top.map((insight) => `<span class="agent-signal-chip ${insight.severity === 'high' ? 'chip-high' : insight.severity === 'warning' ? 'chip-warn' : 'chip-neutral'}">${escapeHtml(insight.title)}</span>`).join('')}
        ${sorted.length > 3 ? `<span class="agent-signal-chip chip-more">+${sorted.length - 3} more</span>` : ''}
      </div>
      <a href="#" class="agent-signal-link" data-page="agents">View all</a>
    </section>
  `;
}

function contextChip<T extends { id: string; name: string }>(label: string, value: string, stateKey: 'selectedSchoolId' | 'selectedLevelId' | 'selectedClassId', options: T[]) {
  const disabled = options.length === 0 ? 'disabled' : '';
  return `
    <label class="context-chip">
      <span>${label}</span>
      <select data-context="${stateKey}" ${disabled}>
        ${options.length === 0
          ? `<option value="">${value}</option>`
          : options.map((option) => `<option value="${escapeHtml(option.id)}" ${state[stateKey] === option.id ? 'selected' : ''}>${escapeHtml(option.name)}</option>`).join('')
        }
      </select>
    </label>
  `;
}

function schoolLogo(school: School | null, className: string) {
  if (school?.logoPath) {
    return `<img class="school-logo ${className}" src="${escapeHtml(school.logoPath)}" alt="${escapeHtml(`${school.name} image`)}" />`;
  }
  const initial = school?.name?.trim().charAt(0).toUpperCase() || 'S';
  return `<span class="school-logo ${className} school-logo-fallback" aria-hidden="true">${escapeHtml(initial)}</span>`;
}

function teacherAvatar(className: string) {
  if (state.teacherProfile.image) {
    return `<img class="${className}" src="${escapeHtml(state.teacherProfile.image)}" alt="${escapeHtml(`${state.teacherProfile.name} profile image`)}" />`;
  }
  const initial = state.teacherProfile.name.trim().charAt(0).toUpperCase() || 'T';
  return `<div class="${className}">${escapeHtml(initial)}</div>`;
}

function render() {
  if (startupInstallGate.active) {
    root.innerHTML = renderStartupInstallGate();
    bindStartupInstallGateEvents();
    return;
  }

  if (state.loading) {
    shell(`<section class="hero-panel"><h2>Loading EduTrack</h2><p>Reading local workspace data.</p></section>`);
    return;
  }

  const views: Record<Page, () => string> = {
    overview: renderOverview,
    classrooms: renderClassrooms,
    syllabus: renderSyllabus,
    lessonPlans: renderLessonPlans,
    calendar: renderCalendar,
    agents: renderAgents,
    tests: renderTests,
    reports: renderReports,
    settings: renderSettings,
  };
  shell(views[state.page]());
  bindPageEvents();
}

// ==================== OVERVIEW PAGE ====================

function renderOverview() {
  const completedSessions = state.sessions.filter((s) => s.completed).length;
  const metrics = [
    { label: 'Schools', value: state.schools.length.toString(), caption: 'Configured schools', iconName: 'book', tone: 'lavender', fill: Math.min(100, state.schools.length * 28) },
    { label: 'Students', value: state.students.length.toString(), caption: selectedClass(state) ? 'Active roster' : 'Select a class', iconName: 'users', tone: 'sky', fill: Math.min(100, state.students.length * 8) },
    { label: 'Classes', value: state.classes.length.toString(), caption: selectedLevel(state) ? 'Active level' : 'Select a level', iconName: 'layout', tone: 'violet', fill: Math.min(100, state.classes.length * 18) },
    { label: 'Average score', value: '-', caption: 'Assessment records needed', iconName: 'check-circle', tone: 'peach', fill: 10 },
    { label: 'Attendance', value: attendanceSummary(), caption: 'Recorded sessions', iconName: 'calendar', tone: 'mint', fill: state.sessions.length > 0 ? Math.round((completedSessions / state.sessions.length) * 100) : 10 },
    { label: 'Teaching hours', value: '-', caption: 'Session duration data needed', iconName: 'cpu', tone: 'rose', fill: 10 },
    { label: 'Syllabus coverage', value: coverageSummary(), caption: 'Generated plans', iconName: 'book', tone: 'violet', fill: selectedClass(state)?.yearPlanId ? 100 : 10 },
    { label: 'Agent alerts', value: state.insights.length.toString(), caption: 'Current class', iconName: 'bot', tone: state.insights.length > 0 ? 'peach' : 'sky', fill: Math.min(100, Math.max(8, state.insights.length * 22)) },
  ];
  const nextSession = state.sessions.find((s) => !s.completed) ?? state.sessions[0];
  return `
    <section class="hero-panel overview-hero">
      <div class="school-hero-title">
        ${schoolLogo(selectedSchool(state), 'school-logo-lg')}
        <div>
          <p class="eyebrow">Dashboard</p>
          <h2 class="text-balance">${selectedSchool(state)?.name ?? 'No school selected'}</h2>
          <p>${selectedLevel(state)?.name ?? 'Choose a level'} ${selectedClass(state) ? `- ${selectedClass(state)?.name}` : ''}</p>
        </div>
      </div>
      <div class="day-stack" aria-label="Teaching day snapshot">
        <article><span>Class pulse</span><strong>${selectedClass(state)?.name ?? 'Select class'}</strong></article>
        <article><span>Next session</span><strong>${nextSession ? escapeHtml(nextSession.title) : 'No session'}</strong></article>
        <article><span>Agent signal</span><strong>${state.insights.length > 0 ? `${state.insights.length} to review` : 'Quiet'}</strong></article>
      </div>
    </section>
    <section class="metric-grid">
      ${metrics.map((metric, index) => `
        <article class="metric-card ${metric.tone} animate-fade-in-up" style="animation-delay: ${index * 50}ms; --metric-fill: ${metric.fill}%">
          <div class="metric-top"><span class="label">${metric.label}</span><span class="metric-icon">${icon(metric.iconName)}</span></div>
          <strong>${metric.value}</strong><p>${metric.caption}</p>
          <span class="metric-bar" aria-hidden="true"></span>
        </article>
      `).join('')}
    </section>
    <section class="panel directory-panel animate-fade-in-up delay-300">
      <div class="panel-head"><div><p class="eyebrow">Directory</p><h2>Created schools and classes</h2></div></div>
      ${renderSchoolDirectory()}
    </section>

  `;
}

function renderSetupWorkflow() {
  const canAddClass = state.schools.length > 0;
  const defaultClassDates = selectedLevel(state)
    ? { start: selectedLevel(state)?.academicYearStart ?? academicYearWindow().start, end: selectedLevel(state)?.academicYearEnd ?? academicYearWindow().end }
    : academicYearWindow();
  return `
    <section class="setup-workflow animate-fade-in-up">
      <div class="workflow-header">
        <div>
          <p class="eyebrow">Start here</p>
          <h2>Add School, then Add Class</h2>
          <p>Create the school first. Once it exists, add classes under that school.</p>
        </div>
        <div class="workflow-steps" aria-label="School setup workflow">
          <span class="active">1 Add school</span>
          <span class="${canAddClass ? 'active' : ''}">2 Add class</span>
        </div>
      </div>
      ${state.setupNotice ? `<div class="notice ${state.setupBusy ? 'notice-info' : 'notice-ok'} setup-notice">${icon(state.setupBusy ? 'alert-circle' : 'check-circle')}<span>${escapeHtml(state.setupNotice)}</span></div>` : ''}
      <div class="setup-grid">
        <article class="panel setup-panel setup-primary">
          <div><p class="eyebrow">Step 1</p><h2>Add school</h2></div>
          <form id="school-form" class="setup-form">
            <label><span class="label">School name</span><input id="school-name" type="text" placeholder="Northview International" required /></label>
            <label class="country-picker-field"><span class="label">Region</span>${countryPickerMarkup()}</label>
            <label><span class="label">Academic year</span><select id="school-year">${academicYearOptions()}</select></label>
            <label class="school-logo-field"><span class="label">School image</span><input id="school-logo" type="file" accept="image/png,image/jpeg,image/webp,image/svg+xml" /><span class="input-hint">Optional logo or school image for the workspace UI.</span></label>
            <div id="school-logo-preview" class="school-logo-preview" aria-live="polite"><span>No image selected</span></div>
            <button class="primary-button" type="submit" ${state.setupBusy ? 'disabled' : ''}>${icon('book')} ${state.setupBusy === 'school' ? 'Adding school...' : 'Add school'}</button>
          </form>
        </article>
        <article class="panel setup-panel ${canAddClass ? '' : 'is-locked'}">
          <div><p class="eyebrow">Step 2</p><h2>Add class</h2></div>
          <form id="class-form" class="setup-form">
            <label><span class="label">School</span><select id="class-school" ${canAddClass ? '' : 'disabled'}>${state.schools.length === 0 ? '<option value="">Create a school first</option>' : state.schools.map((s) => `<option value="${escapeHtml(s.id)}" ${state.selectedSchoolId === s.id ? 'selected' : ''}>${escapeHtml(s.name)}</option>`).join('')}</select></label>
            <label><span class="label">Level / year group</span><input id="class-level" type="text" placeholder="Grade 6" value="${escapeHtml(selectedLevel(state)?.name ?? '')}" ${canAddClass ? '' : 'disabled'} /></label>
            <label><span class="label">Class name</span><input id="class-name" type="text" placeholder="6A" required ${canAddClass ? '' : 'disabled'} /></label>
            <label><span class="label">Subject</span><input id="class-subject" type="text" placeholder="Mathematics" ${canAddClass ? '' : 'disabled'} /></label>
            <label><span class="label">Start date</span><input id="class-start-date" type="date" value="${escapeHtml(defaultClassDates.start)}" ${canAddClass ? '' : 'disabled'} /></label>
            <label><span class="label">Finish date</span><input id="class-end-date" type="date" value="${escapeHtml(defaultClassDates.end)}" ${canAddClass ? '' : 'disabled'} /></label>
            <label><span class="label">Lesson duration (min)</span><input id="class-period-minutes" type="number" min="15" max="180" value="45" ${canAddClass ? '' : 'disabled'} /></label>
            <fieldset class="weekday-field"><legend class="label">Class days</legend><div class="weekday-picker">${weekdays.map((day) => `<label class="weekday-chip"><input type="checkbox" name="class-weekday" value="${day.value}" ${canAddClass ? '' : 'disabled'} /><span>${day.label}</span></label>`).join('')}</div></fieldset>
            <button class="primary-button" type="submit" ${canAddClass && !state.setupBusy ? '' : 'disabled'}>${icon('users')} ${state.setupBusy === 'class' ? 'Adding class...' : 'Add class'}</button>
          </form>
        </article>
      </div>
    </section>
  `;
}

function renderSchoolDirectory() {
  if (state.directory.length === 0) return emptyState('No schools yet', 'Add a school, then create classes under that school.');
  return `<div class="school-directory">${state.directory.map((entry) => {
    const classCount = entry.levels.reduce((count, level) => count + level.classes.length, 0);
    return `<article class="school-row ${state.selectedSchoolId === entry.school.id ? 'active' : ''}">
      <div class="school-row-head">
        <div class="school-row-title">${schoolLogo(entry.school, 'school-logo-md')}<div><strong>${escapeHtml(entry.school.name)}</strong><span>${escapeHtml(entry.school.academicYearLabel ?? 'Academic year not set')} / ${escapeHtml(entry.school.timezone)}</span></div></div>
        <div class="school-row-actions"><button type="button" class="ghost-button compact-button" data-select-school="${escapeHtml(entry.school.id)}">Open</button></div>
      </div>
      <div class="school-meta"><span>${entry.levels.length} levels</span><span>${classCount} classes</span><span>${escapeHtml(countryName(entry.school.regionCode) ?? 'No country')}</span></div>
      ${entry.levels.length === 0 ? '<div class="mini-empty">No classes yet</div>' : `<div class="level-list">${entry.levels.map((level) => `<div class="level-row"><span>${escapeHtml(level.name)}</span><div>${level.classes.length === 0 ? '<em>No classes</em>' : level.classes.map((klass) => `<button type="button" class="class-pill" data-select-school="${escapeHtml(entry.school.id)}" data-select-level="${escapeHtml(level.id)}" data-select-class="${escapeHtml(klass.id)}">${escapeHtml(klass.name)}${klass.subjectName ? ` / ${escapeHtml(klass.subjectName)}` : ''}</button>`).join('')}</div></div>`).join('')}</div>`}
    </article>`;
  }).join('')}</div>`;
}

function attendanceSummary() {
  const sessions = activeClassSessions();
  if (sessions.length === 0) return 'No data';
  const completed = sessions.filter((s) => s.completed).length;
  return `${completed}/${sessions.length}`;
}

function coverageSummary() {
  return selectedClass(state)?.yearPlanId ? 'Plan linked' : 'No data';
}

function activeClassSessions() {
  const klass = selectedClass(state);
  const level = selectedLevel(state);
  const range = klass && level ? classDateRange(klass, level) : null;
  const closureDates = new Set(state.calendarClosures.map((c) => c.closureDate));
  return state.sessions.filter((s) => !closureDates.has(s.sessionDate) && (!range || (s.sessionDate >= range.start && s.sessionDate <= range.end)));
}

function sortedSessions() {
  const today = formatYmdUtc(new Date());
  return activeClassSessions().filter((s) => state.showAttendanceHistory || s.sessionDate >= today).sort((a, b) => a.sessionDate.localeCompare(b.sessionDate));
}

function attendanceFor(studentId: string, sessionId: string) {
  return state.attendanceRecords.find((r) => r.studentId === studentId && r.sessionId === sessionId) ?? null;
}

function isAttendedStatus(status?: string | null) {
  return status === 'present' || status === 'late';
}

function attendanceTone(status: string) {
  if (status === 'present') return 'is-present';
  if (status === 'late') return 'is-late';
  if (status === 'absent') return 'is-absent';
  if (status === 'excused') return 'is-excused';
  if (status === 'ignore') return 'is-ignore';
  return 'is-blank';
}

function attendanceMark(status: string) {
  if (status === 'present') return 'P';
  if (status === 'late') return 'L';
  if (status === 'absent') return 'A';
  if (status === 'excused') return 'E';
  if (status === 'ignore') return 'I';
  return '-';
}

function nextAttendanceStatus(status: string) {
  if (!status) return 'present';
  if (status === 'present') return 'late';
  if (status === 'late') return 'absent';
  if (status === 'absent') return 'excused';
  if (status === 'excused') return 'ignore';
  return 'present';
}

function findSchoolRegion(schoolId: string) {
  return state.schools.find((s) => s.id === schoolId)?.regionCode || 'local';
}

function findLevelId(schoolId: string, levelName: string) {
  const entry = state.directory.find((e) => e.school.id === schoolId);
  return entry?.levels.find((l) => l.name.toLowerCase() === levelName.toLowerCase())?.id ?? '';
}

function nextLevelOrder(schoolId: string) {
  const entry = state.directory.find((e) => e.school.id === schoolId);
  return (entry?.levels.length ?? 0) + 1;
}

// ==================== CLASSROOMS PAGE ====================

function renderClassrooms() {
  return `
    <section class="page-header-card animate-fade-in-up delay-100">
      <div>
        <p class="eyebrow">Classroom</p>
        <h2 class="text-balance">${selectedClass(state)?.name ?? 'No class selected'}</h2>
        <p>Switch between roster, schedule, attendance, and tests.</p>
      </div>
      <div class="segmented">
        ${(['students', 'calendar', 'attendance', 'tests'] as ClassTab[]).map((tab) => `<button type="button" class="${state.classTab === tab ? 'active' : ''}" data-class-tab="${tab}">${capitalize(tab)}</button>`).join('')}
      </div>
    </section>
    <div class="animate-fade-in-up delay-200">${renderClassroomTab()}</div>
  `;
}

function renderClassroomTab() {
  if (!selectedClass(state)) return emptyState('No class selected', 'Create or select a class to use the classroom workspace.');
  if (state.classTab === 'students') return renderStudentsTab();
  if (state.classTab === 'calendar') {
    const html = classCalendar();
    return `<section class="panel class-calendar-panel">${html}</section>`;
  }
  if (state.classTab === 'tests') return renderClassroomTestsTab();
  return `<section class="panel"><div class="panel-head"><div><p class="eyebrow">Attendance register</p><h2>Class attendance</h2></div><button id="toggle-attendance-history" class="ghost-button" type="button">${state.showAttendanceHistory ? 'Hide history' : 'Show history'}</button></div>${attendanceSpreadsheet()}</section>`;
}

function renderStudentsTab() {
  return `
    <section class="panel">
      <div class="panel-head"><div><p class="eyebrow">Roster</p><h2>Students (${state.students.length})</h2></div></div>
      <form id="student-form" class="student-form">
        <label><span class="label">Full name</span><input id="student-full-name" type="text" placeholder="Student name" required /></label>
        <label><span class="label">Preferred name</span><input id="student-preferred-name" type="text" placeholder="Optional" /></label>
        <label><span class="label">Student code</span><input id="student-code" type="text" placeholder="Optional" /></label>
        <label><span class="label">Age</span><input id="student-age" type="number" min="3" max="99" placeholder="Optional" /></label>
        <label><span class="label">Gender</span><select id="student-gender"><option value="">Not specified</option><option value="Boy">Boy</option><option value="Girl">Girl</option></select></label>
        <button class="primary-button" type="submit">${icon('users')} Add student</button>
      </form>
      ${state.students.length === 0 ? emptyState('No students', 'This class has no students yet.') : studentGrid()}
    </section>
  `;
}

function studentGrid() {
  const avatarColors = ['var(--lavender)','var(--mint)','var(--peach)','var(--sky)','var(--rose)','var(--violet)','var(--amber)'];
  const emojis = ['ðŸ‘¦', 'ðŸ‘§', 'ðŸ‘¨â€ðŸŽ“', 'ðŸ‘©â€ðŸŽ“', 'ðŸ§’', 'ðŸ‘±â€â™‚ï¸', 'ðŸ‘±â€â™€ï¸'];
  return `<div class="student-grid">${state.students.map((s, i) => studentCardHtml(s, i, avatarColors, emojis)).join('')}</div>`;
}

function studentCardHtml(s: Student, i: number, avatarColors: string[], emojis: string[]) {
  const color = avatarColors[i % avatarColors.length];
  const emoji = emojis[i % emojis.length];
  const delay = (i % 10) * 50;
  const note = s.notesPrivate ?? '';
  const preferredNameHtml = s.preferredName ? '<span>' + escapeHtml(s.preferredName) + '</span>' : '<span>-</span>';
  const ageHtml = s.age ? '<span class="student-badge age">' + s.age + ' yrs</span>' : '';
  const genderHtml = s.gender ? '<span class="student-badge gender">' + escapeHtml(s.gender) + '</span>' : '';
  return '<article class="student-card animate-fade-in-up" style="animation-delay:' + delay + 'ms">' +
    '<div class="student-card-head">' +
      '<div class="student-avatar" style="background:' + color + '">' + emoji + '</div>' +
      '<div class="student-info">' +
        '<strong>' + escapeHtml(s.fullName) + '</strong>' +
        preferredNameHtml +
      '</div>' +
    '</div>' +
    '<div class="student-badges">' +
      ageHtml +
      genderHtml +
    '</div>' +
    '<div class="student-card-actions">' +
      '<textarea data-qa-note="' + escapeHtml(s.id) + '" rows="1" placeholder="Notes...">' + escapeHtml(note) + '</textarea>' +
      '<button type="button" class="ghost-button compact-button" data-save-note="' + escapeHtml(s.id) + '" title="Save">' + icon('check-circle') + '</button>' +
      '<button type="button" class="danger-button compact-button" data-remove-student="' + escapeHtml(s.id) + '" title="Remove">Remove</button>' +
    '</div>' +
  '</article>';
}

function attendanceSpreadsheet() {
  if (state.students.length === 0) return emptyState('No students', 'Add students before taking attendance.');
  const totalSessions = activeClassSessions();
  if (totalSessions.length === 0) return emptyState('No attendance dates', 'Choose class meeting days and EduTrack will create the date columns.');
  const visibleSessions = sortedSessions();
  if (visibleSessions.length === 0) return emptyState('No upcoming attendance', 'There are no class dates from today onward. Use Show history to view past attendance.');
  return `
    <div class="attendance-sheet-wrap">
      <table class="attendance-sheet">
        <thead><tr><th class="sticky-col student-col">Student</th>${visibleSessions.map((s) => `<th class="date-col"><span>${escapeHtml(shortDate(s.sessionDate))}</span><small>${escapeHtml(s.title)}</small></th>`).join('')}<th class="sticky-total total-col">Attended total</th></tr></thead>
        <tbody>${state.students.map((student, index) => {
          const studentTotal = totalSessions.filter((s) => attendanceFor(student.id, s.id)?.status !== 'ignore').length;
          const attendedTotal = totalSessions.filter((s) => isAttendedStatus(attendanceFor(student.id, s.id)?.status)).length;
          const percent = studentTotal > 0 ? Math.round((attendedTotal / studentTotal) * 100) : 0;
          return `<tr class="${index % 2 === 0 ? 'row-even' : 'row-odd'}">
            <th class="sticky-col student-col" scope="row"><strong>${escapeHtml(student.fullName)}</strong><span>${escapeHtml(student.studentCode ?? student.preferredName ?? 'No code')}</span></th>
            ${visibleSessions.map((session) => {
              const status = attendanceFor(student.id, session.id)?.status ?? '';
              return `<td><button type="button" class="attendance-cell ${attendanceTone(status)}" data-attendance-cell="true" data-session-id="${escapeHtml(session.id)}" data-student-id="${escapeHtml(student.id)}" aria-label="${escapeHtml(`${student.fullName} ${session.sessionDate} ${status || 'not marked'}`)}">${escapeHtml(attendanceMark(status))}</button></td>`;
            }).join('')}
            <td class="sticky-total total-col"><strong>${percent}%</strong><span>${attendedTotal}/${studentTotal}</span></td>
          </tr>`;
        }).join('')}</tbody>
      </table>
    </div>
    <div class="attendance-legend">
      <span><i class="present"></i>Present</span><span><i class="late"></i>Late</span><span><i class="absent"></i>Absent</span>
      <span><i class="excused"></i>Excused</span><span><i class="ignore"></i>Ignore</span><span><i class="blank"></i>Unmarked</span>
    </div>`;
}

function classCalendar() {
  const klass = selectedClass(state);
  const level = selectedLevel(state);
  const cursor = classCalendarCursor();
  const year = cursor.getUTCFullYear();
  const month = cursor.getUTCMonth();
  const monthTitle = cursor.toLocaleDateString(undefined, { month: 'long', year: 'numeric', timeZone: 'UTC' });
  const range = klass && level ? classDateRange(klass, level) : null;
  const academicLabel = range ? `${formatDisplayDate(range.start)} - ${formatDisplayDate(range.end)}` : 'No class dates';
  const scheduledClassDays = activeClassSessions().length;
  return `
    <div class="class-calendar-hero">
      <div>
        <p class="eyebrow">Class calendar</p>
        <h2>${escapeHtml(klass?.name ?? 'No class selected')}</h2>
        <p>${escapeHtml(academicLabel)} - ${scheduledClassDays} scheduled class days, holidays excluded</p>
      </div>
      <div class="class-calendar-controls">
        <div class="segmented">${(['month', 'year'] as ClassCalendarMode[]).map((mode) => `<button type="button" class="${state.classCalendarMode === mode ? 'active' : ''}" data-class-calendar-mode="${mode}">${capitalize(mode)}</button>`).join('')}</div>
        <div class="calendar-nav">
          <button class="ghost-button compact-button" type="button" data-class-calendar-shift="-1">Prev</button>
          <button class="ghost-button compact-button" type="button" data-class-calendar-today="true">Today</button>
          <button class="ghost-button compact-button" type="button" data-class-calendar-shift="1">Next</button>
        </div>
      </div>
    </div>
    <div class="class-calendar-summary">
      <div><span>Weekly rhythm</span><strong>${escapeHtml(classScheduleSummary())}</strong></div>
      <div><span>Lesson plan</span><strong>${state.generatedLessons.length > 0 ? `${state.generatedLessons.length} lessons` : 'No plan linked'}</strong></div>
      <div><span>Flex days</span><strong>${state.generatedLessons.filter((l) => l.isBuffer).length}</strong></div>
      <div><span>Closures</span><strong>${state.calendarClosures.length}</strong></div>
    </div>
    ${state.generatedLessons.some((l) => l.isBuffer) ? '<p class="calendar-footnote">Flex days are review, reteaching, or catch-up time built into the plan automatically.</p>' : ''}
    ${state.classCalendarMode === 'year' ? yearCalendar(year) : `<div class="class-calendar-month-head"><h3>${escapeHtml(monthTitle)}</h3><span>${escapeHtml(selectedSchool(state)?.name ?? 'School calendar')}</span></div>${monthCalendar(year, month)}`}
    ${state.generatedLessons.some((l) => l.teachingDate < formatYmdUtc(new Date()) && l.status !== 'completed') ? `<button id="reschedule-missed" class="primary-button compact-button" type="button">${icon('calendar')} Reschedule missed lessons</button>` : ''}
  `;
}

function classCalendarCursor() {
  return parseYmdUtc(`${state.classCalendarMonth}-01`) ?? new Date(Date.UTC(new Date().getFullYear(), new Date().getMonth(), 1));
}

function shiftClassCalendar(value: number) {
  const cursor = classCalendarCursor();
  cursor.setUTCMonth(cursor.getUTCMonth() + (state.classCalendarMode === 'year' ? value * 12 : value));
  state.classCalendarMonth = `${cursor.getUTCFullYear()}-${`${cursor.getUTCMonth() + 1}`.padStart(2, '0')}`;
}

function classScheduleSummary() {
  if (state.scheduleRules.length === 0) return 'No meeting days set';
  return state.scheduleRules.map((r) => [capitalize(r.weekday), r.periodLabel, r.startTime].filter(Boolean).join(' ')).join(', ');
}

function classCalendarLessons() {
  const classId = state.selectedClassId;
  if (!classId) return state.generatedLessons;
  const lessons = state.allCalendarLessons
    .filter((entry) => entry.classId === classId)
    .map((entry) => entry.lesson);
  return lessons.length > 0 ? lessons : state.generatedLessons;
}

function monthCalendar(year: number, month: number, compact = false) {
  const dates = monthGridDates(year, month);
  return `<div class="month-grid ${compact ? 'mini-month-grid' : ''}">
    ${weekLabels.map((l) => `<div class="weekday-head">${l}</div>`).join('')}
    ${dates.map((date) => {
      const iso = formatYmdUtc(date);
      const inMonth = date.getUTCMonth() === month;
      const events = classCalendarEvents(iso);
      const lessonPill = events.lesson
        ? events.lesson.isBuffer
          ? '<span class="day-pill buffer-pill">Flex</span>'
          : `<span class="day-pill lesson-pill status-${escapeHtml(events.lesson.status)}">${escapeHtml(events.lesson.lessonTitle)}</span>`
        : '';
      const lessonStateBadge = events.lesson && !events.lesson.isBuffer
        ? events.lesson.status === 'completed'
          ? '<span class="lesson-state-badge is-complete">Complete</span>'
          : events.lesson.status === 'missed'
            ? '<span class="lesson-state-badge is-incomplete">Not completed</span>'
            : '<span class="lesson-state-badge is-pending">Pending</span>'
        : '';
      const lessonActions = events.lesson && !events.lesson.isBuffer && !compact
        ? `<div class="lesson-actions">
            <button class="lesson-status-chip complete ${events.lesson.status === 'completed' ? 'active' : ''}" data-mark-lesson="${escapeHtml(events.lesson.id)}" data-status="completed" title="Mark complete">Complete</button>
            <button class="lesson-status-chip incomplete ${events.lesson.status === 'missed' ? 'active' : ''}" data-mark-lesson="${escapeHtml(events.lesson.id)}" data-status="missed" title="Mark not completed">Not completed</button>
          </div>`
        : '';
      const viewPlanButton = events.lesson && events.lesson.detailedPlanAttached && !events.lesson.isBuffer && !compact
        ? `<button class="ghost-button compact-button" type="button" data-open-lesson-plan="${escapeHtml(events.lesson.id)}">View Lesson Plan</button>`
        : '';
      return `<div class="month-day ${inMonth ? '' : 'muted-day'} ${isTodayIso(iso) ? 'today' : ''} ${events.hasClass ? 'has-class' : ''} ${events.closure ? 'has-closure' : ''} ${events.lesson ? 'has-lesson' : ''}" data-date="${escapeHtml(iso)}">
        <div class="day-number">${date.getUTCDate()}</div>
        ${events.hasClass ? '<span class="day-pill class-pill-dot">Class</span>' : ''}
        ${lessonPill}
        ${lessonStateBadge}
        ${lessonActions}
        ${viewPlanButton}
        ${events.closure ? `<span class="day-pill closure-pill">${escapeHtml(events.closure.title ?? 'Closure')}</span>` : ''}
      </div>`;
    }).join('')}
  </div>`;
}

function yearCalendar(year: number) {
  return `<div class="year-calendar-head"><h3>${year}</h3><span>${activeClassSessions().length} scheduled class days in this class, holidays excluded</span></div>
    <div class="year-calendar-grid">${Array.from({ length: 12 }, (_, month) => {
      const date = new Date(Date.UTC(year, month, 1));
      return `<section class="year-month"><h4>${escapeHtml(date.toLocaleDateString(undefined, { month: 'long', timeZone: 'UTC' }))}</h4>${monthCalendar(year, month, true)}</section>`;
    }).join('')}</div>`;
}

function classCalendarEvents(isoDate: string) {
  const closure = state.calendarClosures.find((item) => item.closureDate === isoDate) ?? null;
  const session = closure ? null : activeClassSessions().find((item) => item.sessionDate === isoDate) ?? null;
  const lessons = classCalendarLessons();
  const lesson = closure ? null : lessons.find((item) => item.teachingDate === isoDate) ?? null;
  return { hasClass: Boolean(session), lesson, closure };
}

function dayLessonsListModal(date: string, lessons: Array<{ lesson: YearPlanLesson; className: string; schoolName: string; subjectName: string | null }>) {
  const rows = lessons.map((cl) => {
    const statusLabel = cl.lesson.status === 'completed' ? 'Completed' : cl.lesson.status === 'missed' ? 'Not Completed' : 'Planned';
    const viewAction = cl.lesson.detailedPlanAttached
      ? `<button class="ghost-button compact-button" type="button" data-view-lesson="${escapeHtml(cl.lesson.id)}">View Lesson Plan</button>`
      : '<span style="font-size:11px;color:#64748b">No detailed plan</span>';
    return `<li style="padding:8px 10px;margin:4px 0;background:#f8fafc;border-radius:6px;display:flex;justify-content:space-between;align-items:center;gap:8px">
      <div><strong>${escapeHtml(cl.className)}</strong>${cl.subjectName ? ` Â· ${escapeHtml(cl.subjectName)}` : ''}<br><span style="font-size:12px;color:#475569">${escapeHtml(cl.lesson.lessonTitle)} Â· ${statusLabel}</span></div>
      ${viewAction}
    </li>`;
  }).join('');
  return `<div class="modal-overlay" id="lesson-modal">
    <div class="modal-card" style="max-width:540px">
      <div class="modal-header">
        <div><h3>${escapeHtml(date)}</h3><p>${lessons.length} ${lessons.length === 1 ? 'lesson' : 'lessons'} across all classes</p></div>
        <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
      </div>
      <div class="modal-body"><ul style="list-style:none;padding:0;margin:0">${rows}</ul></div>
    </div>
  </div>`;
}

function dayClassScheduleModal(date: string, sessions: CalendarClassSessionView[]) {
  const rows = sessions.map((cl) => {
    const statusLabel = cl.session.completed ? 'Completed' : 'Scheduled';
    return `<li style="padding:8px 10px;margin:4px 0;background:#f8fafc;border-radius:6px;display:flex;justify-content:space-between;align-items:center;gap:8px">
      <div>
        <strong>${escapeHtml(cl.className)}</strong>${cl.subjectName ? ` Â· ${escapeHtml(cl.subjectName)}` : ''}<br>
        <span style="font-size:12px;color:#475569">${escapeHtml(cl.schoolName)} Â· ${statusLabel}</span>
      </div>
    </li>`;
  }).join('');
  return `<div class="modal-overlay" id="lesson-modal">
    <div class="modal-card" style="max-width:540px">
      <div class="modal-header">
        <div><h3>${escapeHtml(date)}</h3><p>${sessions.length} ${sessions.length === 1 ? 'class session' : 'class sessions'}</p></div>
        <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
      </div>
      <div class="modal-body"><ul style="list-style:none;padding:0;margin:0">${rows}</ul></div>
    </div>
  </div>`;
}

function lessonDetailModal(lesson: YearPlanLesson, className?: string) {
  const plan = parseLessonPlan(lesson.lessonObjective);
  const duration = selectedClass(state)?.periodMinutes ?? 45;
  const statusLabel = lesson.status === 'completed' ? 'Completed' : lesson.status === 'missed' ? 'Not Completed' : 'Planned';
  const statusClass = lesson.status === 'completed' ? 'status-completed' : lesson.status === 'missed' ? 'status-missed' : '';
  const statusActions = lesson.isBuffer
    ? ''
    : `<div class="top-actions" style="margin-top:14px">
        <button class="lesson-btn completed-btn" data-mark-lesson="${escapeHtml(lesson.id)}" data-status="completed" title="Mark completed">${icon('check-circle')} Done</button>
        <button class="lesson-btn missed-btn" data-mark-lesson="${escapeHtml(lesson.id)}" data-status="missed" title="Mark not completed">${icon('alert-circle')} Not completed</button>
      </div>`;
  let bodyHtml = '';
  if (!plan) {
    if (lesson.isBuffer) {
      bodyHtml = `<p><strong>Flex day:</strong> this date is reserved for review, reteaching, or catch-up. You can generate activities for this day while keeping it marked as flex for later rescheduling.</p>`;
    } else {
      bodyHtml = `<p>${escapeHtml(lesson.lessonObjective ?? 'No lesson plan details available.')}</p>`;
    }
  } else {
    const flexDayNote = lesson.isBuffer
      ? `<p><strong>Flex day:</strong> this stays marked as a flex/catch-up day, but now includes optional classroom activities you can run if no makeup lesson is needed.</p>`
      : '';
    const backwardDesign = (plan as unknown as { backwardDesign?: unknown }).backwardDesign;
    const backwardDesignBlock = backwardDesign && typeof backwardDesign === 'object'
      ? `<div class="bd-section"><h4>Backward Design Template</h4><details open><summary class="label">Full structured plan</summary><pre style="white-space:pre-wrap;line-height:1.35;font-size:12px;color:#334155;background:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;padding:10px;max-height:360px;overflow:auto">${escapeHtml(JSON.stringify(backwardDesign, null, 2))}</pre></details></div>`
      : '';
    bodyHtml = `
      ${flexDayNote}
      <div class="bd-section"><h4>Learning Objective</h4><p>${escapeHtml(plan.objective)}</p></div>
      <div class="bd-section"><h4>Transfer Goal</h4><p>${escapeHtml(plan.transferGoal)}</p></div>
      <div class="bd-section"><h4>Enduring Understanding</h4><p>${escapeHtml(plan.enduringUnderstanding)}</p></div>
      <div class="bd-section"><h4>Essential Vocabulary</h4><ul>${plan.vocabulary.map((v) => `<li>${escapeHtml(v)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Guiding Questions</h4><ul>${plan.essentialQuestions.map((q) => `<li>${escapeHtml(q)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Knowledge</h4><ul>${plan.knowledge.map((k) => `<li>${escapeHtml(k)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Skills</h4><ul>${plan.skills.map((s) => `<li>${escapeHtml(s)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Success Criteria</h4><ul>${plan.successCriteria.map((c) => `<li>${escapeHtml(c)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Assessment Evidence</h4><p>${escapeHtml(plan.assessment)}</p>${plan.performanceTask ? `<p><strong>Performance Task:</strong> ${escapeHtml(plan.performanceTask)}</p>` : ''}</div>
      <div class="bd-section"><h4>Learning Plan</h4><table class="flow-table"><thead><tr><th>Stage</th><th>Time</th><th>Activity</th></tr></thead><tbody>${plan.lessonFlow.map((step) => `<tr><td>${escapeHtml(step.phase)}</td><td>${escapeHtml(step.time)}</td><td>${escapeHtml(step.description)}</td></tr>`).join('')}</tbody></table></div>
      <div class="bd-section"><h4>Materials</h4><ul>${plan.materials.map((m) => `<li>${escapeHtml(m)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Differentiation</h4><p>${escapeHtml(plan.differentiation).replace(/\n/g, '<br>')}</p></div>
      ${plan.homework ? `<div class="bd-section"><h4>Homework / Follow-Up</h4><p>${escapeHtml(plan.homework)}</p></div>` : ''}
      ${plan.crossCurricular ? `<div class="bd-section"><h4>Cross-Curricular Connections</h4><p>${escapeHtml(plan.crossCurricular)}</p></div>` : ''}
      ${backwardDesignBlock}
    `;
  }
  return `<div class="modal-overlay" id="lesson-modal">
    <div class="modal-card">
      <div class="modal-header">
        <div>
          <h3>${escapeHtml(lesson.lessonTitle)}</h3>
          <p>${className ? escapeHtml(className) + ' Â· ' : ''}${escapeHtml(lesson.teachingDate)} Â· ${escapeHtml(capitalize(lesson.weekday))} Â· ${duration} min Â· <span class="${statusClass}">${statusLabel}</span></p>
        </div>
        <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
      </div>
      <div class="modal-body">${bodyHtml}${statusActions}</div>
    </div>
  </div>`;
}

// ==================== SYLLABUS PAGE ====================

function extractionConfidenceLabel(confidence: number) {
  if (confidence >= 85) return 'High';
  if (confidence >= 70) return 'Good';
  if (confidence >= 50) return 'Medium';
  return 'Low';
}

function syllabusDiagnosticsPanel(diagnostics: SyllabusExtractionDiagnostics) {
  return `
    <div class="panel" style="margin-top:var(--space-3)">
      <div class="panel-head"><h2>Extraction confidence</h2><button id="syllabus-refresh-diagnostics" class="ghost-button compact-button" type="button">${icon('cpu')} Re-check</button></div>
      <p class="settings-detail" style="margin:0 0 var(--space-2) 0">
        ${extractionConfidenceLabel(diagnostics.confidence)} confidence (${diagnostics.confidence}%) • Layout: ${escapeHtml(diagnostics.detectedLayout.replace(/_/g, ' '))} • Units: ${diagnostics.unitCount}
      </p>
      <p class="settings-detail" style="margin:0 0 6px 0"><strong style="color:var(--ink-strong)">Interpreter notes</strong></p>
      <ul class="rich-list">${diagnostics.notes.map((note) => `<li>${escapeHtml(note)}</li>`).join('')}</ul>
      <p class="settings-detail" style="margin:8px 0 6px 0"><strong style="color:var(--ink-strong)">Suggested actions</strong></p>
      <ul class="rich-list">${diagnostics.suggestedActions.map((action) => `<li>${escapeHtml(action)}</li>`).join('')}</ul>
      <div style="margin-top:var(--space-2);display:flex;gap:6px">
        <button id="syllabus-export-text" class="ghost-button compact-button" type="button">${icon('upload')} Export raw text</button>
      </div>
    </div>
  `;
}

function renderSyllabus() {
  const canGenerateDetailed = !state.lessonPlansBusy;
  const canAttachDetailed = !state.lessonPlansBusy;
  const detailedPlanBlockers: string[] = [];
  if (!state.backend) detailedPlanBlockers.push('Open the desktop app to run AI lesson planning.');
  if (!state.selectedClassId) detailedPlanBlockers.push('Select a class in the Syllabus filters.');
  if (nonBufferLessons().length === 0) detailedPlanBlockers.push('Generate a lesson schedule first.');
  if (!state.llmConfig?.available) detailedPlanBlockers.push('Connect and save a local model in Settings.');
  const progressPercent = detailedPlanProgressPercent();
  return `
    <section class="syllabus-page">
      <article class="syllabus-upload-bar">
        <div class="syllabus-upload-main">
          <div class="syllabus-filters">
            <label class="filter-chip"><span>School</span><select id="syllabus-school" ${state.schools.length > 0 ? '' : 'disabled'}>${state.schools.length === 0 ? '<option value="">No schools</option>' : state.schools.map((s) => `<option value="${escapeHtml(s.id)}" ${state.selectedSchoolId === s.id ? 'selected' : ''}>${escapeHtml(s.name)}</option>`).join('')}</select></label>
            <label class="filter-chip"><span>Level</span><select id="syllabus-level" ${state.levels.length > 0 ? '' : 'disabled'}>${state.levels.length === 0 ? '<option value="">No levels</option>' : state.levels.map((l) => `<option value="${escapeHtml(l.id)}" ${state.selectedLevelId === l.id ? 'selected' : ''}>${escapeHtml(l.name)}</option>`).join('')}</select></label>
            <label class="filter-chip"><span>Class</span><select id="syllabus-class" ${state.classes.length > 0 ? '' : 'disabled'}>${state.classes.length === 0 ? '<option value="">No classes</option>' : state.classes.map((k) => `<option value="${escapeHtml(k.id)}" ${state.selectedClassId === k.id ? 'selected' : ''}>${escapeHtml(k.name)}${k.subjectName ? ` / ${escapeHtml(k.subjectName)}` : ''}</option>`).join('')}</select></label>
          </div>
          <input id="syllabus-file" type="file" accept=".pdf,.txt,.md,.csv" ${selectedLevel(state) ? '' : 'disabled'} />
        </div>
        <div class="syllabus-upload-actions">
          <button id="upload-syllabus" class="primary-button" type="button" ${selectedLevel(state) && selectedClass(state) && !state.syllabusBusy ? '' : 'disabled'}>${state.syllabusBusy ? '<span class="spinner" style="width:16px;height:16px;border-width:2px;margin:0"></span>' : icon('upload')} ${state.syllabusBusy ? 'Processing...' : 'Upload'}</button>
        </div>
      </article>
      ${state.syllabusBusy
        ? `<div class="syllabus-loading animate-fade-in-up"><div class="spinner"></div><p>Reading syllabus and extracting editable sections...</p></div>`
        : state.planMessage
          ? `<div class="notice notice-info animate-fade-in-up">${icon('check-circle')}<span>${escapeHtml(state.planMessage)}</span></div>`
          : ''
      }
      ${state.syllabusBusy || state.extractedUnits.length === 0 || !state.syllabusExtractionDiagnostics
        ? ''
        : syllabusDiagnosticsPanel(state.syllabusExtractionDiagnostics)
      }
      ${state.syllabusBusy ? '' : state.extractedUnits.length === 0
        ? `<div class="panel" style="margin-top:var(--space-3)">${emptyState('No extracted units', 'Upload a PDF or text syllabus using the bar above. Extracted units will appear here for review before generating a lesson plan.')}</div>`
        : syllabusReviewList(canGenerateDetailed, canAttachDetailed, detailedPlanBlockers)
      }
      ${state.detailedPlanProgressActive ? `
        <div class="modal-overlay loading-overlay" data-detailed-plan-progress-overlay="1">
          <div class="modal-card loading-card">
            <div class="modal-header">
              <div>
                <h3>Generating Detailed Lesson Plans</h3>
                <p>Please wait while EduTrack generates and saves detailed lesson plans for the full year.</p>
              </div>
            </div>
            <div class="modal-body">
              <div class="syllabus-loading"><div class="spinner"></div></div>
              <div class="progress-track"><span data-detailed-plan-progress-bar style="width:${progressPercent}%"></span></div>
              <p class="settings-detail" data-detailed-plan-progress-count>Progress: ${state.detailedPlanProgressCompleted} / ${state.detailedPlanProgressTotal} lessons completed</p>
              <p class="settings-detail" data-detailed-plan-progress-message>Current task: ${escapeHtml(state.detailedPlanProgressMessage || 'Preparing generation')}</p>
              <p class="settings-detail" data-detailed-plan-progress-percent>Completed ${progressPercent}%</p>
              <div style="display:flex;justify-content:flex-end;margin-top:12px">
                <button id="cancel-detailed-plan-generation" class="danger-button" type="button" ${detailedPlanCancelRequested ? 'disabled' : ''}>${detailedPlanCancelRequested ? 'Cancelling...' : 'Cancel Generation'}</button>
              </div>
            </div>
          </div>
        </div>
      ` : ''}
    </section>
  `;
}

function syllabusReviewList(canGenerateDetailed: boolean, canAttachDetailed: boolean, detailedPlanBlockers: string[]) {
  return `
    <div class="panel" style="margin-top:var(--space-3);padding:0;overflow:hidden">
      <div class="syllabus-review-header"><p class="eyebrow">Syllabus sections</p><span>${state.extractedUnits.length} ${state.extractedUnits.length === 1 ? 'unit' : 'units'}</span></div>
      <div class="syllabus-units-grid">${state.extractedUnits.map((unit, index) => `
        <div class="syllabus-unit-card">
          <div class="unit-card-head">
            <span class="unit-badge">${index + 1}</span>
            <span class="unit-code">${escapeHtml(unit.unitCode ?? `UNIT-${String(index + 1).padStart(3, '0')}`)}</span>
            <label class="unit-required-toggle" title="Required"><input type="checkbox" data-unit-required="${index}" ${unit.isRequired ? 'checked' : ''} /></label>
          </div>
          <input type="text" class="unit-title-input" data-unit-title="${index}" value="${escapeHtml(unit.title)}" placeholder="Unit title" />
          <textarea class="unit-desc-input" data-unit-description="${index}" rows="2" placeholder="Description">${escapeHtml(unit.description ?? '')}</textarea>
          <div class="unit-meta-row">
            <label class="unit-meta-field"><span>Lessons</span><input type="number" min="1" max="40" data-unit-lessons="${index}" value="${unit.estimatedLessons}" /></label>
            <label class="unit-meta-field"><span>Assessment</span><input type="text" data-unit-assessment="${index}" value="${escapeHtml(unit.assessmentHint ?? '')}" placeholder="e.g. Quiz, Project" /></label>
          </div>
          <div class="unit-assigned-range"><span>Assigned timeline</span><strong>${escapeHtml(unitAssignedRangeLabel(unit))}</strong></div>
        </div>
      `).join('')}</div>
      <div class="syllabus-review-footer">
        <button id="generate-reviewed-plan" class="primary-button" type="button">${icon('book')} Generate Lesson Schedule</button>
        <div class="schedule-options-row">
          <label class="unit-meta-field"><span>Buffer %</span><input id="schedule-buffer-percent" type="number" min="0" max="25" step="1" value="8" title="Percentage of teaching days reserved as buffer/reteach days" /></label>
          <label class="unit-meta-field"><span>Pacing</span><select id="schedule-pacing-mode" title="Controls how lessons are distributed across the calendar"><option value="balanced" selected>Balanced</option><option value="front_loaded">Front-loaded</option><option value="back_loaded">Back-loaded</option></select></label>
        </div>
        <button id="syllabus-generate-detailed-plans" class="ghost-button" type="button" ${canGenerateDetailed ? '' : 'disabled'}>${state.lessonPlansBusy ? '<span class="spinner"></span>' : icon('cpu')} ${state.lessonPlansBusy ? 'Generating...' : 'Generate Detailed Lesson Plans'}</button>
        <button id="syllabus-attach-detailed-plans" class="ghost-button" type="button" ${canAttachDetailed ? '' : 'disabled'}>${icon('calendar')} Attach Detailed Plans to Calendar Lessons</button>
      </div>
      ${detailedPlanBlockers.length > 0
        ? `<div class="settings-detail" style="padding:0 18px 14px 18px">Detailed plans are not ready yet: ${escapeHtml(detailedPlanBlockers[0])}</div>`
        : ''}
    </div>
  `;
}

function collectReviewedUnits() {
  return state.extractedUnits.flatMap((unit, index) => {
    const included = document.querySelector<HTMLInputElement>(`[data-unit-required="${index}"]`)?.checked ?? unit.isRequired;
    if (!included) return [];
    const estimatedLessons = Number(document.querySelector<HTMLInputElement>(`[data-unit-lessons="${index}"]`)?.value || unit.estimatedLessons);
    return [{
      ...unit,
      title: document.querySelector<HTMLInputElement>(`[data-unit-title="${index}"]`)?.value.trim() || unit.title,
      description: document.querySelector<HTMLTextAreaElement>(`[data-unit-description="${index}"]`)?.value.trim() || null,
      estimatedLessons: Number.isFinite(estimatedLessons) ? Math.max(1, Math.min(40, estimatedLessons)) : unit.estimatedLessons,
      estimatedWeeks: Math.max(1, (Number.isFinite(estimatedLessons) ? estimatedLessons : unit.estimatedLessons)),
      assessmentHint: document.querySelector<HTMLInputElement>(`[data-unit-assessment="${index}"]`)?.value.trim() || null,
      isRequired: true,
    }];
  });
}

function classroomSubmissionFor(testId: string, studentId: string) {
  const rows = state.classroomTestSubmissions[testId] ?? [];
  return rows.find((row) => row.studentId === studentId) ?? null;
}

function renderClassroomTestsTab() {
  if (state.tests.length === 0) {
    return `<section class="panel">${emptyState('No tests created', 'Create tests in the main Tests page, then upload student files here.')}</section>`;
  }
  if (state.students.length === 0) {
    return `<section class="panel">${emptyState('No students in class', 'Add students first, then upload test submissions in this matrix.')}</section>`;
  }
  return `
    <section class="panel classroom-tests-panel">
      <div class="panel-head">
        <div><p class="eyebrow">Classroom tests</p><h2>Student uploads by test</h2></div>
        <span>${state.students.length} students · ${state.tests.length} tests</span>
      </div>
      <div class="classroom-tests-matrix-wrap">
        <table class="classroom-tests-matrix">
          <thead>
            <tr>
              <th>Student</th>
              ${state.tests.map((test) => `<th>${escapeHtml(test.title)}</th>`).join('')}
            </tr>
          </thead>
          <tbody>
            ${state.students.map((student) => `
              <tr>
                <th>${escapeHtml(student.fullName)}</th>
                ${state.tests.map((test) => {
                  const sub = classroomSubmissionFor(test.id, student.id);
                  const fileLabel = sub?.fileName ? `<span class="submission-file">${escapeHtml(sub.fileName)}</span>` : '<span class="submission-file empty">No file</span>';
                  const scoreLabel = sub?.scoreValue != null ? `<span class="test-row-status completed">Scored ${sub.scoreValue}</span>` : '<span class="test-row-status pending">Pending</span>';
                  return `<td>
                    <div class="classroom-test-cell">
                      ${fileLabel}
                      ${scoreLabel}
                      <input type="file" data-upload-matrix="1" data-matrix-assessment="${escapeHtml(test.id)}" data-matrix-student="${escapeHtml(student.id)}" accept=".pdf,.doc,.docx,.txt,.md,.csv,.json,.xml,.html,.htm,image/*,audio/*" />
                    </div>
                  </td>`;
                }).join('')}
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>
      <p class="settings-detail">Use the main Tests page for creating tests and running full auto-scoring/export.</p>
    </section>
  `;
}

function collectSyllabusSectionsForTestDraft(): TestSyllabusSection[] {
  return state.extractedUnits.map((unit, index) => {
    const requiredInput = document.querySelector<HTMLInputElement>(`[data-unit-required="${index}"]`);
    const titleInput = document.querySelector<HTMLInputElement>(`[data-unit-title="${index}"]`);
    const descriptionInput = document.querySelector<HTMLTextAreaElement>(`[data-unit-description="${index}"]`);
    const lessonsInput = document.querySelector<HTMLInputElement>(`[data-unit-lessons="${index}"]`);
    const assessmentInput = document.querySelector<HTMLInputElement>(`[data-unit-assessment="${index}"]`);
    const parsedLessons = Number(lessonsInput?.value ?? unit.estimatedLessons);
    return {
      id: unit.id,
      unitCode: unit.unitCode ?? null,
      title: titleInput?.value.trim() || unit.title,
      description: descriptionInput?.value.trim() || unit.description || null,
      estimatedLessons: Number.isFinite(parsedLessons) ? Math.max(1, Math.min(40, parsedLessons)) : Math.max(1, unit.estimatedLessons),
      assessmentHint: assessmentInput?.value.trim() || unit.assessmentHint || null,
      isRequired: requiredInput?.checked ?? Boolean(unit.isRequired),
    };
  });
}

function openTestSyllabusSectionPickerModal(sections: TestSyllabusSection[]): Promise<TestSyllabusSection[] | null> {
  return new Promise((resolve) => {
    const modalId = 'test-syllabus-picker-modal';
    document.getElementById(modalId)?.remove();

    const rows = sections.map((section, index) => {
      const code = section.unitCode?.trim() || `UNIT-${String(index + 1).padStart(3, '0')}`;
      const description = (section.description ?? '').trim();
      const assessment = (section.assessmentHint ?? '').trim();
      return `
        <label class="diag-row" style="grid-template-columns:auto 1fr;align-items:start;cursor:pointer">
          <input type="checkbox" data-test-syllabus-check="${index}" ${section.isRequired ? 'checked' : ''} />
          <div>
            <strong>${escapeHtml(code)} - ${escapeHtml(section.title)}</strong>
            <p>${escapeHtml(description || 'No description')}</p>
            <p>Lessons: ${section.estimatedLessons}${assessment ? ` | Assessment: ${escapeHtml(assessment)}` : ''}</p>
          </div>
        </label>
      `;
    }).join('');

    const wrapper = document.createElement('div');
    wrapper.innerHTML = `
      <div class="modal-overlay" id="${modalId}">
        <div class="modal-card" style="max-width:760px">
          <div class="modal-header">
            <div>
              <h3>Create test from syllabus</h3>
              <p>Select the syllabus sections to include in this test.</p>
            </div>
            <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
          </div>
          <div class="modal-body">
            <div style="display:flex;gap:8px;justify-content:flex-end;margin-bottom:10px">
              <button type="button" class="ghost-button compact-button" data-test-syllabus-select-all="1">Select all</button>
              <button type="button" class="ghost-button compact-button" data-test-syllabus-clear-all="1">Clear all</button>
            </div>
            <div class="diag-grid">${rows}</div>
            <div style="display:flex;justify-content:flex-end;gap:10px;margin-top:12px">
              <button type="button" class="ghost-button" data-modal-cancel="1">Cancel</button>
              <button type="button" class="primary-button" data-modal-confirm="1">${icon('book')} Build draft</button>
            </div>
          </div>
        </div>
      </div>
    `;
    const modal = wrapper.firstElementChild as HTMLElement | null;
    if (!modal) {
      resolve(null);
      return;
    }
    document.body.appendChild(modal);

    const close = (result: TestSyllabusSection[] | null) => {
      modal.remove();
      resolve(result);
    };
    const checkboxes = () => Array.from(modal.querySelectorAll<HTMLInputElement>('[data-test-syllabus-check]'));

    modal.querySelector<HTMLElement>('.modal-close')?.addEventListener('click', () => close(null));
    modal.querySelector<HTMLElement>('[data-modal-cancel]')?.addEventListener('click', () => close(null));
    modal.querySelector<HTMLElement>('[data-test-syllabus-select-all]')?.addEventListener('click', () => {
      for (const box of checkboxes()) box.checked = true;
    });
    modal.querySelector<HTMLElement>('[data-test-syllabus-clear-all]')?.addEventListener('click', () => {
      for (const box of checkboxes()) box.checked = false;
    });
    modal.querySelector<HTMLElement>('[data-modal-confirm]')?.addEventListener('click', () => {
      const selected = checkboxes()
        .filter((box) => box.checked)
        .map((box) => Number(box.dataset.testSyllabusCheck))
        .filter((idx) => Number.isFinite(idx) && idx >= 0 && idx < sections.length)
        .map((idx) => ({ ...sections[idx], isRequired: true }));
      close(selected);
    });
    modal.addEventListener('click', (event) => {
      if (event.target === modal) close(null);
    });
  });
}

function buildDetailedPlanningContext(lessons: YearPlanLesson[]) {
  const teachingLessons = lessons.filter((lesson) => !lesson.isBuffer);
  if (teachingLessons.length === 0) return '';

  const unitCounts = new Map<string, number>();
  const monthUnits = new Map<string, Map<string, number>>();

  for (const lesson of teachingLessons) {
    const unit = lesson.lessonTitle.split(':')[0]?.trim() || lesson.lessonTitle.trim() || 'General progression';
    unitCounts.set(unit, (unitCounts.get(unit) ?? 0) + 1);
    const monthKey = lesson.teachingDate.slice(0, 7);
    const monthMap = monthUnits.get(monthKey) ?? new Map<string, number>();
    monthMap.set(unit, (monthMap.get(unit) ?? 0) + 1);
    monthUnits.set(monthKey, monthMap);
  }

  const topUnits = Array.from(unitCounts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 8)
    .map(([unit, count]) => `${unit} (${count} lessons)`);

  const monthLines = Array.from(monthUnits.entries())
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([month, units]) => {
      const focus = Array.from(units.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 4)
        .map(([unit, count]) => `${unit} (${count})`)
        .join(', ');
      return `${month}: ${focus}`;
    });

  const context = [
    'Year pacing context for this class:',
    `Year goal trajectory: ${topUnits.join(', ')}`,
    'Month-by-month focus goals:',
    ...monthLines,
    'Keep each lesson aligned to this yearly and monthly progression while preserving backward design structure.',
  ].join('\n');

  return context.slice(0, 3200);
}

type DetailedGenerationOptions = {
  templateContent: string | null;
  templateName: string;
  contextNotes: string | null;
  createFlexActivities: boolean;
};

function pickTemplateFile() {
  return new Promise<File | null>((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.txt,.md,.html,.htm';
    input.addEventListener('change', () => resolve(input.files?.[0] ?? null), { once: true });
    input.click();
  });
}

function openConfirmModal(options: {
  id?: string;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  confirmClass?: 'primary' | 'danger' | 'ghost';
}): Promise<boolean> {
  return new Promise((resolve) => {
    const modalId = options.id ?? 'app-confirm-modal';
    const existing = document.getElementById(modalId);
    if (existing) existing.remove();

    const confirmClassName = options.confirmClass === 'danger'
      ? 'danger-button'
      : options.confirmClass === 'ghost'
        ? 'ghost-button'
        : 'primary-button';

    const wrapper = document.createElement('div');
    wrapper.innerHTML = `
      <div class="modal-overlay" id="${modalId}">
        <div class="modal-card" style="max-width:520px">
          <div class="modal-header">
            <div>
              <h3>${escapeHtml(options.title)}</h3>
              <p>${escapeHtml(options.message)}</p>
            </div>
            <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
          </div>
          <div class="modal-body">
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="ghost-button" type="button" data-modal-cancel="1">${escapeHtml(options.cancelLabel ?? 'Cancel')}</button>
              <button class="${confirmClassName}" type="button" data-modal-confirm="1">${escapeHtml(options.confirmLabel ?? 'Confirm')}</button>
            </div>
          </div>
        </div>
      </div>
    `;

    const modal = wrapper.firstElementChild as HTMLElement | null;
    if (!modal) {
      resolve(false);
      return;
    }
    document.body.appendChild(modal);

    const close = (result: boolean) => {
      modal.remove();
      resolve(result);
    };

    modal.querySelector<HTMLElement>('.modal-close')?.addEventListener('click', () => close(false));
    modal.querySelector<HTMLElement>('[data-modal-cancel]')?.addEventListener('click', () => close(false));
    modal.querySelector<HTMLElement>('[data-modal-confirm]')?.addEventListener('click', () => close(true));
    modal.addEventListener('click', (event) => {
      if (event.target === modal) close(false);
    });
  });
}

function openTextEntryModal(options: {
  id?: string;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  placeholder?: string;
}): Promise<string | null> {
  return new Promise((resolve) => {
    const modalId = options.id ?? 'app-text-entry-modal';
    const existing = document.getElementById(modalId);
    if (existing) existing.remove();

    const wrapper = document.createElement('div');
    wrapper.innerHTML = `
      <div class="modal-overlay" id="${modalId}">
        <div class="modal-card" style="max-width:560px">
          <div class="modal-header">
            <div>
              <h3>${escapeHtml(options.title)}</h3>
              <p>${escapeHtml(options.message)}</p>
            </div>
            <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
          </div>
          <div class="modal-body">
            <input id="app-text-entry-input" type="text" placeholder="${escapeHtml(options.placeholder ?? '')}" />
            <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:12px">
              <button class="ghost-button" type="button" data-modal-cancel="1">${escapeHtml(options.cancelLabel ?? 'Cancel')}</button>
              <button class="danger-button" type="button" data-modal-confirm="1">${escapeHtml(options.confirmLabel ?? 'Confirm')}</button>
            </div>
          </div>
        </div>
      </div>
    `;

    const modal = wrapper.firstElementChild as HTMLElement | null;
    if (!modal) {
      resolve(null);
      return;
    }
    document.body.appendChild(modal);

    const input = modal.querySelector<HTMLInputElement>('#app-text-entry-input');
    if (input) input.focus();

    const close = (value: string | null) => {
      modal.remove();
      resolve(value);
    };

    modal.querySelector<HTMLElement>('.modal-close')?.addEventListener('click', () => close(null));
    modal.querySelector<HTMLElement>('[data-modal-cancel]')?.addEventListener('click', () => close(null));
    modal.querySelector<HTMLElement>('[data-modal-confirm]')?.addEventListener('click', () => close(input?.value ?? ''));
    input?.addEventListener('keydown', (event) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        close(input.value);
      }
    });
    modal.addEventListener('click', (event) => {
      if (event.target === modal) close(null);
    });
  });
}

function openDetailedPlanOptionsModal(): Promise<{
  uploadTemplate: boolean;
  addContext: boolean;
  contextText: string;
  createFlexActivities: boolean;
} | null> {
  return new Promise((resolve) => {
    const existing = document.getElementById('detailed-plan-options-modal');
    if (existing) existing.remove();

    const wrapper = document.createElement('div');
    wrapper.innerHTML = `
      <div class="modal-overlay" id="detailed-plan-options-modal">
        <div class="modal-card" style="max-width:560px">
          <div class="modal-header">
            <div>
              <h3>Generate Detailed Lesson Plans</h3>
              <p>Choose options before starting full-year generation.</p>
            </div>
            <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
          </div>
          <div class="modal-body">
            <label style="display:flex;align-items:flex-start;gap:10px;margin:0 0 12px 0;cursor:pointer">
              <input id="dp-option-template" type="checkbox" />
              <span>
                <strong style="display:block;color:var(--ink-strong)">Upload a lesson plan template</strong>
                <span class="settings-detail" style="margin:0">Use a .txt, .md, or .html template to guide structure and tone.</span>
              </span>
            </label>
            <label style="display:flex;align-items:flex-start;gap:10px;margin:0 0 10px 0;cursor:pointer">
              <input id="dp-option-context" type="checkbox" ${state.lessonContextNotes.trim() ? 'checked' : ''} />
              <span>
                <strong style="display:block;color:var(--ink-strong)">Add extra context or instructions</strong>
                <span class="settings-detail" style="margin:0">Optional notes about class needs, teaching style, or focus areas.</span>
              </span>
            </label>
            <label style="display:flex;align-items:flex-start;gap:10px;margin:0 0 10px 0;cursor:pointer">
              <input id="dp-option-flex-activities" type="checkbox" />
              <span>
                <strong style="display:block;color:var(--ink-strong)">Create games and class activities for flex days</strong>
                <span class="settings-detail" style="margin:0">Generate activity plans only for flex days. Days stay marked as flex for future makeup lessons.</span>
              </span>
            </label>
            <textarea id="dp-context-input" class="lesson-context-input" rows="5" placeholder="Enter extra context or instructions for lesson plans..." ${state.lessonContextNotes.trim() ? '' : 'style="display:none"'}>${escapeHtml(state.lessonContextNotes || '')}</textarea>
            <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:14px">
              <button id="dp-cancel" class="ghost-button" type="button">Cancel</button>
              <button id="dp-start" class="primary-button" type="button">${icon('cpu')} Start generation</button>
            </div>
          </div>
        </div>
      </div>
    `;
    const modal = wrapper.firstElementChild as HTMLElement | null;
    if (!modal) {
      resolve(null);
      return;
    }
    document.body.appendChild(modal);

    const close = (value: { uploadTemplate: boolean; addContext: boolean; contextText: string; createFlexActivities: boolean } | null) => {
      modal.remove();
      resolve(value);
    };

    const templateToggle = modal.querySelector<HTMLInputElement>('#dp-option-template');
    const contextToggle = modal.querySelector<HTMLInputElement>('#dp-option-context');
    const flexActivitiesToggle = modal.querySelector<HTMLInputElement>('#dp-option-flex-activities');
    const contextInput = modal.querySelector<HTMLTextAreaElement>('#dp-context-input');
    const startButton = modal.querySelector<HTMLButtonElement>('#dp-start');
    const cancelButton = modal.querySelector<HTMLButtonElement>('#dp-cancel');
    const closeButton = modal.querySelector<HTMLButtonElement>('.modal-close');

    const syncContextVisibility = () => {
      if (!contextInput || !contextToggle) return;
      contextInput.style.display = contextToggle.checked ? 'block' : 'none';
    };

    contextToggle?.addEventListener('change', syncContextVisibility);
    syncContextVisibility();

    startButton?.addEventListener('click', () => {
      close({
        uploadTemplate: templateToggle?.checked ?? false,
        addContext: contextToggle?.checked ?? false,
        contextText: contextInput?.value ?? '',
        createFlexActivities: flexActivitiesToggle?.checked ?? false,
      });
    });
    cancelButton?.addEventListener('click', () => close(null));
    closeButton?.addEventListener('click', () => close(null));
    modal.addEventListener('click', (event) => {
      if (event.target === modal) close(null);
    });
  });
}

async function promptDetailedPlanOptions(): Promise<DetailedGenerationOptions | null> {
  const modalResult = await openDetailedPlanOptionsModal();
  if (!modalResult) return null;

  let templateContent: string | null = null;
  let templateName = '';
  if (modalResult.uploadTemplate) {
    const templateFile = await pickTemplateFile();
    if (!templateFile) {
      showToast('Template upload cancelled. Continuing with Backward Design format.');
    } else {
      try {
        templateContent = await templateFile.text();
        templateName = templateFile.name;
      } catch {
        showToast('Template upload failed. Please try uploading the template again.');
        return null;
      }
    }
  }

  let contextNotes: string | null = null;
  if (modalResult.addContext) {
    const trimmed = modalResult.contextText.trim();
    contextNotes = trimmed.length > 0 ? trimmed : null;
    state.lessonContextNotes = modalResult.contextText;
    try { localStorage.setItem('edutrack.lessonContextNotes', state.lessonContextNotes); } catch { /* ignore */ }
  }

  return { templateContent, templateName, contextNotes, createFlexActivities: modalResult.createFlexActivities };
}

async function refreshSyllabusExtractionDiagnostics(syllabusId: string) {
  if (!state.backend || !syllabusId) {
    state.syllabusExtractionDiagnostics = null;
    return;
  }
  try {
    const diagnostics = await invoke<SyllabusExtractionDiagnostics>('get_syllabus_extraction_diagnostics', { syllabusId });
    state.syllabusExtractionDiagnostics = diagnostics ?? null;
  } catch {
    state.syllabusExtractionDiagnostics = null;
  }
}

async function generateDetailedPlansFromSyllabus() {
  if (!state.backend) {
    showToast('Open the desktop app to generate detailed lesson plans');
    return;
  }
  if (!state.selectedClassId) {
    showToast('No lesson schedule found. Please generate a lesson schedule before generating detailed lesson plans.');
    return;
  }
  if (!state.llmConfig?.available) {
    showToast('Connect and save a local model in Settings before generating detailed lesson plans.');
    return;
  }
  if (isCloudBackedModel(state.llmConfig?.model)) {
    showToast('Note: Using a cloud/quota-limited model. Generation may be slower due to rate-limit pacing.');
  }
  const lessons = nonBufferLessons();
  const flexLessons = sortedGeneratedLessons().filter((lesson) => lesson.isBuffer);
  if (lessons.length === 0 && flexLessons.length === 0) {
    showToast('No lesson schedule found. Please generate a lesson schedule before generating detailed lesson plans.');
    return;
  }
  if (state.lessonPlansBusy) return;

  const options = await promptDetailedPlanOptions();
  if (!options) return;

  if (options.templateContent) {
    state.lessonTemplate = options.templateContent;
    state.lessonTemplateName = options.templateName || 'Uploaded template';
  } else {
    state.lessonTemplate = '';
    state.lessonTemplateName = '';
  }
  if (typeof options.contextNotes === 'string') {
    state.lessonContextNotes = options.contextNotes;
  }
  if (options.createFlexActivities && flexLessons.length === 0) {
    showToast('No flex days found in this schedule. Regenerate the year plan with buffer days first.');
    return;
  }

  const targetLessons = options.createFlexActivities ? flexLessons : lessons;
  const generationNoun = options.createFlexActivities ? 'flex-day activity plans' : 'detailed lesson plans';
  const generationItem = options.createFlexActivities ? 'flex day' : 'lesson';

  detailedPlanCancelRequested = false;
  state.lessonPlansBusy = true;
  state.detailedPlanProgressActive = true;
  state.detailedPlanProgressTotal = targetLessons.length;
  state.detailedPlanProgressCompleted = 0;
  state.detailedPlanProgressMessage = 'Preparing lesson plan generation';
  render();
  await renderDetailedPlanProgressTick();

  try {
    const perLessonCooldownMs = 2500;
    let completed = 0;
    let saved = 0;
    let cancelled = false;
    let stoppedByProviderLimit = false;
    let providerLimitMessage = '';
    const sortedLessons = [...targetLessons].sort((a, b) => a.sequenceNumber - b.sequenceNumber);
    const autoPlanningContext = buildDetailedPlanningContext([...lessons].sort((a, b) => a.sequenceNumber - b.sequenceNumber));
    const flexActivityContext = options.createFlexActivities
      ? [
        'Flex-day generation mode:',
        'Generate engaging games and class activities for a full lesson duration.',
        'Keep these days marked as flex/catch-up days so they can still be replaced by rescheduled missed lessons.',
        'Anchor each activity set to the syllabus and yearly pacing context.',
      ].join('\n')
      : '';
    const combinedContextNotes = [state.lessonContextNotes?.trim(), autoPlanningContext, flexActivityContext]
      .filter((item) => Boolean(item && item.length > 0))
      .join('\n\n');
    const failedLessons: string[] = [];
    for (const lesson of sortedLessons) {
      if (detailedPlanCancelRequested) {
        cancelled = true;
        break;
      }
      const generatingLabel = `Generating ${generationItem} ${completed + 1} of ${sortedLessons.length}`;
      state.detailedPlanProgressMessage = generatingLabel;
      await renderDetailedPlanProgressTick();
      const stopHeartbeat = startDetailedPlanHeartbeat(() => generatingLabel);
      let updatedLesson: YearPlanLesson | null = null;
      try {
        updatedLesson = await invoke<YearPlanLesson>('generate_detailed_lesson_plan_for_lesson', {
          classId: state.selectedClassId,
          lessonId: lesson.id,
          contextNotes: combinedContextNotes || null,
          templateContent: state.lessonTemplate || null,
          generateForFlexDayActivities: options.createFlexActivities,
        });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        failedLessons.push(`${capitalize(generationItem)} ${lesson.sequenceNumber}: ${message}`);
        const lower = message.toLowerCase();
        if (lower.includes('429') || lower.includes('too many requests') || lower.includes('rate limit')) {
          stoppedByProviderLimit = true;
          providerLimitMessage = compactProviderError(message);
          detailedPlanCancelRequested = true;
        }
      } finally {
        stopHeartbeat();
      }
      if (updatedLesson) {
        const idx = state.generatedLessons.findIndex((item) => item.id === updatedLesson.id);
        if (idx >= 0) state.generatedLessons[idx] = updatedLesson;
        saved += 1;
      }
      completed += 1;
      state.detailedPlanProgressCompleted = completed;
      state.detailedPlanProgressMessage = `Saving ${generationItem} plan ${completed} of ${sortedLessons.length}`;
      await renderDetailedPlanProgressTick();
      if (detailedPlanCancelRequested) {
        cancelled = !stoppedByProviderLimit;
        break;
      }
      if (completed < sortedLessons.length) {
        state.detailedPlanProgressMessage = `Cooldown before next AI request (${Math.ceil(perLessonCooldownMs / 1000)}s)`;
        await renderDetailedPlanProgressTick();
        await sleepMs(perLessonCooldownMs);
      }
    }
    if (stoppedByProviderLimit) {
      await loadAllCalendarLessons(state);
      await loadAllCalendarSessions(state);
      state.planMessage = `${capitalize(generationNoun)} generation stopped at ${generationItem} ${state.detailedPlanProgressCompleted + 1} of ${sortedLessons.length} due to AI provider rate limit. Saved ${saved} AI-generated ${generationItem} plans. ${providerLimitMessage}`;
      persistSyllabusPlanningState();
      showToast('AI provider rate limit reached. Generation paused to keep plans AI-only.');
      return;
    }
    if (failedLessons.length > 0) {
      await loadAllCalendarLessons(state);
      await loadAllCalendarSessions(state);
      const preview = failedLessons.slice(0, 2).join(' | ');
      state.planMessage = `${capitalize(generationNoun)} generation completed with errors. Saved ${saved} of ${sortedLessons.length} ${generationItem} plans. ${preview}`;
      persistSyllabusPlanningState();
      showToast(`Some ${generationItem} plans failed (${failedLessons.length}). See Syllabus message for details.`);
      return;
    }
    if (cancelled) {
      state.detailedPlanProgressMessage = 'Cancellation requested. Saving completed lesson plans';
      await renderDetailedPlanProgressTick();
      await loadAllCalendarLessons(state);
      await loadAllCalendarSessions(state);
      state.planMessage = `${capitalize(generationNoun)} generation cancelled after ${state.detailedPlanProgressCompleted} of ${sortedLessons.length} ${generationItem}s. Completed plans were saved.`;
      persistSyllabusPlanningState();
      showToast(`${capitalize(generationNoun)} generation cancelled. Completed plans were saved.`);
      return;
    }
    state.detailedPlanProgressMessage = 'Finalising yearly lesson plan set';
    await renderDetailedPlanProgressTick();
    await loadAllCalendarLessons(state);
    await loadAllCalendarSessions(state);
    try {
      await invoke<AttachDetailedPlansResult>('attach_detailed_plans_to_calendar_lessons', {
        classId: state.selectedClassId,
      });
    } catch { /* auto-attach is best-effort */ }
    state.planMessage = options.createFlexActivities
      ? `Flex-day activity plans generated for ${sortedLessons.length} flex days. These days remain marked as flex and can still be replaced by rescheduled missed lessons.`
      : `Detailed lesson plans generated and attached for ${sortedLessons.length} lessons.`;
    persistSyllabusPlanningState();
    showToast(options.createFlexActivities ? 'Flex-day activity plans generated and saved successfully.' : 'Detailed lesson plans generated and saved successfully.');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    state.planMessage = `Detailed lesson plan generation failed after ${state.detailedPlanProgressCompleted} lessons: ${message}`;
    persistSyllabusPlanningState();
    showToast(message || 'Lesson plan generation was interrupted. Please restart the generation process.');
  } finally {
    detailedPlanCancelRequested = false;
    state.lessonPlansBusy = false;
    state.detailedPlanProgressActive = false;
    state.detailedPlanProgressTotal = 0;
    state.detailedPlanProgressCompleted = 0;
    state.detailedPlanProgressMessage = '';
    render();
  }
}

async function attachDetailedPlansToCalendar() {
  if (!state.backend || !state.selectedClassId) {
    showToast('Open the desktop app to attach detailed plans.');
    return;
  }
  try {
    const result = await invoke<AttachDetailedPlansResult>('attach_detailed_plans_to_calendar_lessons', {
      classId: state.selectedClassId,
    });
    if ((result?.totalDetailedCount ?? 0) === 0) {
      showToast('No detailed lesson plans found. Please generate detailed lesson plans before attaching them to calendar lessons.');
      return;
    }
    await loadClassData(state);
    await loadAllCalendarLessons(state);
    await loadAllCalendarSessions(state);
    state.planMessage = `Attached ${result?.attachedCount ?? 0} detailed lesson plans to calendar lessons${(result?.alreadyAttachedCount ?? 0) > 0 ? ` (${result?.alreadyAttachedCount ?? 0} were already attached)` : ''}.`;
    persistSyllabusPlanningState();
    render();
    if ((result?.unmatchedCount ?? 0) > 0) {
      showToast('Some lesson plans could not be attached to calendar lessons. Please review the unmatched items.');
      return;
    }
    showToast('Detailed lesson plans attached to calendar lessons successfully.');
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error));
  }
}

// ==================== CALENDAR PAGE ====================

function renderCalendar() {
  const cursor = calendarCursor();
  const year = cursor.getUTCFullYear();
  const month = cursor.getUTCMonth();
  const allClosures = collectAllClosures();
  const allSessions = state.allCalendarSessions;
  const sessionCount = allSessions.length;
  const completedCount = allSessions.filter((entry) => entry.session.completed).length;
  const schoolCount = new Set(allSessions.map((entry) => entry.schoolName)).size;
  const classCount = new Set(allSessions.map((entry) => entry.className)).size;
  return `
    <section class="page-header-card">
      <div><p class="eyebrow">Master Calendar</p><h2>All classes</h2><p>${schoolCount} schools, ${classCount} classes â€” ${sessionCount} scheduled class sessions, ${completedCount} completed.</p></div>
      <div class="calendar-actions">
        <div class="segmented">${(['month', 'week', 'timeline'] as CalendarMode[]).map((mode) => `<button type="button" class="${state.calendarMode === mode ? 'active' : ''}" data-calendar-mode="${mode}">${capitalize(mode)}</button>`).join('')}</div>
        <div class="calendar-nav">
          <button class="ghost-button compact-button" type="button" data-calendar-shift="-1">Prev</button>
          <button class="ghost-button compact-button" type="button" data-calendar-today="true">Today</button>
          <button class="ghost-button compact-button" type="button" data-calendar-shift="1">Next</button>
        </div>
      </div>
    </section>
    <section class="panel calendar-panel">${calendarContent(allSessions, allClosures, year, month)}</section>
  `;
}

function calendarCursor() {
  return parseYmdUtc(`${state.classCalendarMonth}-01`) ?? new Date(Date.UTC(new Date().getFullYear(), new Date().getMonth(), 1));
}

function shiftCalendar(value: number) {
  const cursor = calendarCursor();
  cursor.setUTCMonth(cursor.getUTCMonth() + value);
  state.classCalendarMonth = `${cursor.getUTCFullYear()}-${`${cursor.getUTCMonth() + 1}`.padStart(2, '0')}`;
}

function collectAllClosures() {
  const closures: Array<{ closureDate: string; title: string | null; closureType: string }> = [];
  for (const c of state.calendarClosures) {
    closures.push({ closureDate: c.closureDate, title: c.title ?? null, closureType: c.closureType });
  }
  return closures;
}

function calendarContent(allSessions: CalendarClassSessionView[], allClosures: Array<{ closureDate: string; title: string | null; closureType: string }>, year: number, month: number) {
  if (state.calendarMode === 'week') return calendarWeekView(allSessions, allClosures);
  if (state.calendarMode === 'timeline') return calendarTimelineView(allSessions, allClosures);
  return calendarMonthView(allSessions, allClosures, year, month);
}

function calendarMonthView(allSessions: CalendarClassSessionView[], allClosures: Array<{ closureDate: string; title: string | null; closureType: string }>, year: number, month: number) {
  const dates = monthGridDates(year, month);
  const todayIso = formatYmdUtc(new Date());
  return `<div class="calendar-month-head"><h3>${escapeHtml(new Date(Date.UTC(year, month)).toLocaleDateString(undefined, { month: 'long', year: 'numeric', timeZone: 'UTC' }))}</h3></div>
    <div class="month-grid">
      ${weekLabels.map((l) => `<div class="weekday-head">${l}</div>`).join('')}
      ${dates.map((date) => {
        const iso = formatYmdUtc(date);
        const inMonth = date.getUTCMonth() === month;
        const dayClosures = allClosures.filter((c) => c.closureDate === iso);
        const daySessions = allSessions.filter((entry) => entry.session.sessionDate === iso);
        const classPills = daySessions.map((cl) => {
          const statusClass = cl.session.completed ? 'status-completed' : '';
          const colorSeed = hashColor(cl.className + cl.schoolName);
          return `<span class="cal-class-pill ${statusClass}" style="--class-color:${colorSeed}">${escapeHtml(cl.className)}${cl.session.completed ? ' âœ“' : ''}</span>`;
        });
        const closurePills = dayClosures.map((c) => `<span class="cal-class-pill closure-pill">${escapeHtml(c.title ?? c.closureType)}</span>`);
        return `<div class="month-day ${inMonth ? '' : 'muted-day'} ${iso === todayIso ? 'today' : ''} ${daySessions.length > 0 ? 'has-lesson' : ''} ${dayClosures.length > 0 ? 'has-closure' : ''}" data-date="${escapeHtml(iso)}">
          <div class="day-number">${date.getUTCDate()}</div>
          ${classPills.join('')}
          ${closurePills.join('')}
        </div>`;
      }).join('')}
    </div>`;
}

function calendarWeekView(allSessions: CalendarClassSessionView[], allClosures: Array<{ closureDate: string; title: string | null; closureType: string }>) {
  const today = new Date();
  const weekStart = new Date(today);
  weekStart.setDate(today.getDate() - today.getDay());
  const days: string[] = [];
  for (let i = 0; i < 7; i++) {
    const d = new Date(weekStart);
    d.setDate(weekStart.getDate() + i);
    days.push(formatYmdUtc(d));
  }
  return `<div class="calendar-board two-col">
    <section class="calendar-section">
      <div class="panel-head compact-head"><div><p class="eyebrow">This week</p><h3>${escapeHtml(days[0])} â€“ ${escapeHtml(days[6])}</h3></div></div>
      ${days.map((d) => {
        const dayLessons = allSessions.filter((cl) => cl.session.sessionDate === d);
        const dayClosures = allClosures.filter((c) => c.closureDate === d);
        const items = dayLessons.map((cl) => {
          const statusLabel = cl.session.completed ? 'Completed' : 'Scheduled';
          return `<li><strong>${escapeHtml(cl.className)}</strong><span>${escapeHtml(statusLabel)}${cl.subjectName ? ` Â· ${escapeHtml(cl.subjectName)}` : ''}</span></li>`;
        }).join('');
        const closures = dayClosures.map((c) => `<li class="closure-item"><strong>${escapeHtml(d)}</strong><span>${escapeHtml(c.title ?? 'Closure')}</span></li>`).join('');
        return `<article class="week-day-card"><h4>${escapeHtml(d)}</h4>${items || closures || '<p class="muted">No classes</p>'}</article>`;
      }).join('')}
    </section>
    <section class="calendar-section"><div class="panel-head compact-head"><div><p class="eyebrow">All classes</p><h3>Overview</h3></div></div>${allClassesOverview(allSessions)}</section>
  </div>`;
}

function calendarTimelineView(allSessions: CalendarClassSessionView[], _allClosures: Array<{ closureDate: string; title: string | null; closureType: string }>) {
  if (allSessions.length === 0) return emptyState('No classes scheduled', 'Add classes and schedules to see them here.');
  const sorted = [...allSessions].sort((a, b) => a.session.sessionDate.localeCompare(b.session.sessionDate));
  return `<div class="calendar-board">
    <section class="calendar-section">
      <div class="panel-head compact-head"><div><p class="eyebrow">Timeline</p><h3>All scheduled classes</h3></div></div>
      <ul class="rich-list lesson-list">${sorted.slice(0, 50).map((cl) => {
        const statusLabel = cl.session.completed ? 'Completed' : 'Scheduled';
        return `<li><strong>${escapeHtml(cl.session.sessionDate)}</strong><span>${escapeHtml(cl.className)}${cl.subjectName ? ` Â· ${escapeHtml(cl.subjectName)}` : ''}</span><p>${escapeHtml(statusLabel)}</p></li>`;
      }).join('')}</ul>
      ${sorted.length > 50 ? `<p class="settings-detail">Showing first 50 of ${sorted.length} class entries.</p>` : ''}
    </section>
  </div>`;
}

function allClassesOverview(allSessions: CalendarClassSessionView[]) {
  const classMap = new Map<string, { className: string; schoolName: string; sessionCount: number; completedCount: number }>();
  for (const cl of allSessions) {
    const key = cl.schoolName + '///' + cl.className;
    const entry = classMap.get(key) ?? { className: cl.className, schoolName: cl.schoolName, sessionCount: 0, completedCount: 0 };
    entry.sessionCount++;
    if (cl.session.completed) entry.completedCount++;
    classMap.set(key, entry);
  }
  const entries = Array.from(classMap.values()).sort((a, b) => a.schoolName.localeCompare(b.schoolName) || a.className.localeCompare(b.className));
  if (entries.length === 0) return emptyState('No classes', 'Add classes and set meeting days to populate the master calendar.');
  return `<ul class="rich-list compact-list">${entries.map((e) => `<li><strong>${escapeHtml(e.className)}</strong><span>${escapeHtml(e.schoolName)} Â· ${e.sessionCount} sessions, ${e.completedCount} completed</span></li>`).join('')}</ul>`;
}

function hashColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) hash = str.charCodeAt(i) + ((hash << 5) - hash);
  const hues = [180, 200, 220, 160, 280, 140, 300, 320, 100, 120, 60, 80];
  return `hsl(${hues[Math.abs(hash) % hues.length]}, 50%, 45%)`;
}



// ==================== LESSON PLANS PAGE ====================

function renderLessonPlans() {
  const lessons = sortedGeneratedLessons();
  const groups = groupLessonsForExport(lessons, state.lessonExportScope);
  syncLessonExportSelection(groups, state.lessonExportScope);
  const selectedCount = selectedLessonExportGroups(groups, state.lessonExportScope).length;
  const detailedCount = lessons.filter((lesson) => parseLessonPlan(lesson.lessonObjective)).length;
  return `
    <section class="lesson-plans-grid">
      <article class="panel lesson-export-panel animate-fade-in-up delay-100">
        <p class="eyebrow">Lesson plans</p>
        <h2>Export teaching plans</h2>
        <p>Review generated plans, then export by lesson, week, month, or year.</p>
        <div class="class-calendar-summary">
          <div><span>School</span><strong>${escapeHtml(selectedSchool(state)?.name ?? 'No school')}</strong></div>
          <div><span>Class</span><strong>${escapeHtml(selectedClass(state)?.name ?? 'No class')}</strong></div>
          <div><span>Lessons</span><strong>${lessons.length}</strong></div>
          <div><span>Detailed</span><strong>${detailedCount}/${lessons.filter((lesson) => !lesson.isBuffer).length}</strong></div>
        </div>
        <p class="settings-detail">Use the Syllabus page to generate and attach detailed lesson plans.</p>
        <div class="segmented lesson-scope-control">${(['lesson', 'week', 'month', 'year'] as LessonExportScope[]).map((scope) => `<button type="button" class="${state.lessonExportScope === scope ? 'active' : ''}" data-lesson-export-scope="${scope}">${capitalize(scope)}</button>`).join('')}</div>
        <div class="lesson-format-row"><span class="label">Format</span><div class="segmented">${(['html', 'md', 'txt', 'docx', 'pdf'] as LessonExportFormat[]).map((fmt) => `<button type="button" class="${state.lessonExportFormat === fmt ? 'active' : ''}" data-lesson-export-format="${fmt}">${fmt.toUpperCase()}</button>`).join('')}</div></div>
      </article>
      <article class="panel animate-fade-in-up delay-200">
        <p class="eyebrow">Export</p>
        <h2>${escapeHtml(exportScopeTitle(state.lessonExportScope))}</h2>
        ${groups.length > 0
          ? `<div class="lesson-export-toolbar"><label class="lesson-select-all"><input id="lesson-select-all" type="checkbox" ${selectedCount === groups.length ? 'checked' : ''} /><span>Select all</span></label><button id="export-selected-groups" class="primary-button compact-button" type="button" ${selectedCount > 0 ? '' : 'disabled'}>${icon('upload')} Export selected (${selectedCount})</button></div>${lessonExportPreview(groups, state.lessonExportScope)}`
          : emptyState('No lesson plan', 'Upload a syllabus, review sections, and generate a class plan first.')}
      </article>
    </section>
  `;
}

function sortedGeneratedLessons() {
  return [...state.generatedLessons].sort((a, b) => a.teachingDate.localeCompare(b.teachingDate));
}

function exportScopeTitle(scope: LessonExportScope) {
  if (scope === 'lesson') return 'By lesson';
  if (scope === 'week') return 'By week';
  if (scope === 'month') return 'By month';
  return 'Full year';
}

function lessonExportPreview(
  groups: Array<{ key: string; title: string; lessons: YearPlanLesson[] }>,
  scope: LessonExportScope,
) {
  return `<div class="lesson-preview-list">${groups.map((group, index) => {
    const selectionId = exportGroupSelectionId(scope, group, index);
    const checked = lessonExportSelection.has(selectionId);
    return `<div class="lesson-preview-row ${checked ? 'selected' : ''}">
      <label class="lesson-group-select"><input type="checkbox" data-export-select="${escapeHtml(selectionId)}" ${checked ? 'checked' : ''} /><span></span></label>
      <div class="lesson-preview-info">${renderLessonPreviewHeading(group, scope)}<span>${group.lessons.length} ${group.lessons.length === 1 ? 'lesson' : 'lessons'}</span><p>${escapeHtml(lessonPreviewSummary(group, scope))}</p></div>
      <div class="lesson-preview-actions"><button type="button" class="ghost-button compact-button" data-preview-export="${escapeHtml(selectionId)}">Preview</button></div>
    </div>`;
  }).join('')}</div>`;
}

function exportGroupPreviewModal(
  group: { key: string; title: string; lessons: YearPlanLesson[] },
  scope: LessonExportScope,
) {
  const sorted = [...group.lessons].sort((a, b) => a.teachingDate.localeCompare(b.teachingDate));
  const first = sorted[0];
  if (!first) {
    return `<div class="modal-overlay" id="lesson-modal">
      <div class="modal-card" style="max-width:540px">
        <div class="modal-header">
          <div><h3>Preview unavailable</h3><p>No lessons in this export group.</p></div>
          <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
        </div>
      </div>
    </div>`;
  }
  if (scope === 'lesson') return lessonDetailModal(first, selectedClass(state)?.name);
  const scopeLabel = scope === 'week' ? 'week' : scope === 'month' ? 'month' : 'year';
  const base = lessonDetailModal(first, selectedClass(state)?.name);
  const note = `<p class="settings-detail" style="margin-bottom:10px">Previewing the first lesson in this ${scopeLabel} export group (${group.lessons.length} lessons total).</p>`;
  return base.replace('<div class="modal-body">', `<div class="modal-body">${note}`);
}

function groupLessonsForExport(lessons: YearPlanLesson[], scope: LessonExportScope) {
  if (scope === 'lesson') return lessons.map((l) => ({ key: l.teachingDate, title: `${l.teachingDate} - ${l.lessonTitle}`, lessons: [l] }));
  const groups = new Map<string, YearPlanLesson[]>();
  for (const lesson of lessons) {
    const date = parseYmdUtc(lesson.teachingDate);
    const key = scope === 'week' ? weekKey(date ?? new Date(`${lesson.teachingDate}T00:00:00`)) : scope === 'month' ? lesson.teachingDate.slice(0, 7) : lesson.teachingDate.slice(0, 4);
    groups.set(key, [...(groups.get(key) ?? []), lesson]);
  }
  return Array.from(groups.entries()).map(([key, gl]) => ({ key, title: exportGroupTitle(key, scope), lessons: gl }));
}

function exportGroupTitle(key: string, scope: LessonExportScope) {
  if (scope === 'week') return `Week of ${key}`;
  if (scope === 'month') { const d = parseYmdUtc(`${key}-01`); return d ? d.toLocaleDateString(undefined, { month: 'long', year: 'numeric', timeZone: 'UTC' }) : key; }
  if (scope === 'year') return key;
  return key;
}

function exportGroupSelectionId(
  scope: LessonExportScope,
  group: { key: string; title: string },
  index: number,
) {
  return `${scope}::${group.key}::${index}::${group.title}`;
}

function syncLessonExportSelection(
  groups: Array<{ key: string; title: string; lessons: YearPlanLesson[] }>,
  scope: LessonExportScope,
) {
  const valid = new Set(groups.map((group, index) => exportGroupSelectionId(scope, group, index)));
  for (const id of Array.from(lessonExportSelection)) {
    if (!valid.has(id)) lessonExportSelection.delete(id);
  }
}

function selectedLessonExportGroups(
  groups: Array<{ key: string; title: string; lessons: YearPlanLesson[] }>,
  scope: LessonExportScope,
) {
  return groups.filter((group, index) => lessonExportSelection.has(exportGroupSelectionId(scope, group, index)));
}

function splitUnitAndTopic(lessonTitle: string) {
  const [unitPart, ...topicParts] = lessonTitle.split(':');
  const unit = unitPart.trim();
  const topic = topicParts.join(':').trim();
  return { unit, topic };
}

function previewUnitTags(lessons: YearPlanLesson[], limit = 3) {
  const seen = new Set<string>();
  const tags: string[] = [];
  for (const lesson of lessons) {
    const { unit } = splitUnitAndTopic(lesson.lessonTitle);
    const normalized = unit.trim();
    if (!normalized || seen.has(normalized)) continue;
    seen.add(normalized);
    tags.push(normalized);
    if (tags.length >= limit) break;
  }
  return tags;
}

function scopedPeriodLabel(group: { key: string; title: string }, scope: LessonExportScope) {
  if (scope === 'week') {
    const start = parseYmdUtc(group.key);
    if (!start) return group.title;
    const end = new Date(start);
    end.setUTCDate(end.getUTCDate() + 6);
    const startIso = formatYmdUtc(start);
    const endIso = formatYmdUtc(end);
    return `${shortDate(startIso)} - ${shortDate(endIso)}`;
  }
  if (scope === 'month') {
    const date = parseYmdUtc(`${group.key}-01`);
    return date ? date.toLocaleDateString(undefined, { month: 'long', year: 'numeric', timeZone: 'UTC' }) : group.title;
  }
  if (scope === 'year') return group.key;
  return group.title;
}

function renderLessonPreviewHeading(
  group: { key: string; title: string; lessons: YearPlanLesson[] },
  scope: LessonExportScope,
) {
  if (scope !== 'lesson') {
    const scopeLabel = scope === 'week' ? 'Week' : scope === 'month' ? 'Month' : 'Year';
    const period = scopedPeriodLabel(group, scope);
    const tags = previewUnitTags(group.lessons, 3);
    return `<div class="lesson-preview-head"><span class="lesson-chip scope">${escapeHtml(scopeLabel)}</span><span class="lesson-chip date">${escapeHtml(period)}</span></div><strong class="lesson-topic">${escapeHtml(group.title)}</strong>${tags.length > 0 ? `<div class="lesson-unit-pills">${tags.map((tag) => `<span class="lesson-chip unit">${escapeHtml(tag)}</span>`).join('')}</div>` : ''}`;
  }
  const lesson = group.lessons[0];
  if (!lesson) return `<strong>${escapeHtml(group.title)}</strong>`;
  const { unit, topic } = splitUnitAndTopic(lesson.lessonTitle);
  return `<div class="lesson-preview-head"><span class="lesson-chip date">${escapeHtml(lesson.teachingDate)}</span><span class="lesson-chip unit">${escapeHtml(unit || lesson.lessonTitle)}</span>${topic ? `<strong class="lesson-topic">${escapeHtml(topic)}</strong>` : ''}</div>`;
}

function lessonPreviewSummary(
  group: { key: string; title: string; lessons: YearPlanLesson[] },
  scope: LessonExportScope,
) {
  if (scope === 'lesson') return 'Single lesson export group';
  return group.lessons.slice(0, 3).map((lesson) => lesson.lessonTitle).join(', ');
}

function humanizePlanKey(key: string) {
  const normalized = key
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/[_-]+/g, ' ')
    .trim();
  return normalized.charAt(0).toUpperCase() + normalized.slice(1);
}

function isPlanRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function renderBackwardDesignText(value: unknown, indent = ''): string {
  if (Array.isArray(value)) {
    return value
      .map((item) => {
        if (isPlanRecord(item) || Array.isArray(item)) {
          return `${indent}-\n${renderBackwardDesignText(item, `${indent}  `)}`;
        }
        return `${indent}- ${String(item ?? '')}`;
      })
      .join('\n');
  }
  if (isPlanRecord(value)) {
    return Object.entries(value)
      .map(([key, item]) => {
        if (isPlanRecord(item) || Array.isArray(item)) {
          return `${indent}${humanizePlanKey(key)}:\n${renderBackwardDesignText(item, `${indent}  `)}`;
        }
        return `${indent}${humanizePlanKey(key)}: ${String(item ?? '')}`;
      })
      .join('\n');
  }
  return `${indent}${String(value ?? '')}`;
}

function renderBackwardDesignMarkdown(value: unknown, depth = 0): string {
  if (Array.isArray(value)) {
    return value
      .map((item) => {
        if (isPlanRecord(item) || Array.isArray(item)) return `- \n${renderBackwardDesignMarkdown(item, depth + 1)}`;
        return `- ${String(item ?? '')}`;
      })
      .join('\n');
  }
  if (isPlanRecord(value)) {
    return Object.entries(value)
      .map(([key, item]) => {
        const heading = `${'#'.repeat(Math.min(6, depth + 4))} ${humanizePlanKey(key)}`;
        if (isPlanRecord(item) || Array.isArray(item)) return `${heading}\n\n${renderBackwardDesignMarkdown(item, depth + 1)}`;
        return `${heading}\n\n${String(item ?? '')}`;
      })
      .join('\n\n');
  }
  return String(value ?? '');
}

function renderBackwardDesignHtml(value: unknown): string {
  if (Array.isArray(value)) {
    return `<ul>${value.map((item) => `<li>${isPlanRecord(item) || Array.isArray(item) ? renderBackwardDesignHtml(item) : escapeHtml(String(item ?? ''))}</li>`).join('')}</ul>`;
  }
  if (isPlanRecord(value)) {
    return Object.entries(value)
      .map(([key, item]) => `<div class="bd-subsection"><h5>${escapeHtml(humanizePlanKey(key))}</h5>${isPlanRecord(item) || Array.isArray(item) ? renderBackwardDesignHtml(item) : `<p>${escapeHtml(String(item ?? ''))}</p>`}</div>`)
      .join('');
  }
  return `<p>${escapeHtml(String(value ?? ''))}</p>`;
}

function lessonTextContent(lesson: YearPlanLesson): string {
  const plan = parseLessonPlan(lesson.lessonObjective);
  let txt = `\n${lesson.teachingDate} Â· ${capitalize(lesson.weekday)}\n  ${lesson.lessonTitle}`;
  if (plan) {
    txt += `\n  Objective: ${plan.objective}`;
    txt += `\n  Transfer: ${plan.transferGoal}`;
    txt += `\n  Assessment: ${plan.assessment}`;
    txt += `\n  Homework: ${plan.homework ?? 'N/A'}`;
    if (plan.backwardDesign) {
      txt += `\n  Backward Design:\n${renderBackwardDesignText(plan.backwardDesign, '    ')}`;
    }
  } else {
    txt += `\n  Objective: ${lesson.lessonObjective ?? 'To be added.'}`;
  }
  txt += `\n`;
  return txt;
}

function lessonMdContent(lesson: YearPlanLesson): string {
  const plan = parseLessonPlan(lesson.lessonObjective);
  let md = `### ${lesson.teachingDate} Â· ${capitalize(lesson.weekday)}\n\n**${lesson.lessonTitle}**\n\n`;
  if (plan) {
    md += `**Objective:** ${plan.objective}\n\n`;
    md += `**Transfer Goal:** ${plan.transferGoal}\n\n`;
    md += `**Enduring Understanding:** ${plan.enduringUnderstanding}\n\n`;
    md += `**Essential Questions:**\n${plan.essentialQuestions.map((q: string) => `- ${q}`).join('\n')}\n\n`;
    md += `**Students will know:**\n${plan.knowledge.map((k: string) => `- ${k}`).join('\n')}\n\n`;
    md += `**Students will be able to:**\n${plan.skills.map((s: string) => `- ${s}`).join('\n')}\n\n`;
    md += `**Assessment:** ${plan.assessment}\n\n`;
    md += `**Lesson Flow:**\n${plan.lessonFlow.map((s: { phase: string; time: string; description: string }) => `- *${s.time}* **${s.phase}**: ${s.description}`).join('\n')}\n\n`;
    md += `**Differentiation:** ${plan.differentiation}\n\n`;
    if (plan.homework) md += `**Homework:** ${plan.homework}\n\n`;
    if (plan.backwardDesign) md += `#### Backward Design\n\n${renderBackwardDesignMarkdown(plan.backwardDesign)}\n\n`;
  } else {
    md += `**Objective:** ${lesson.lessonObjective ?? 'To be added.'}\n\n`;
  }
  return md;
}

function buildLessonExport(context: { school: string; className: string; level: string; scope: string }, groups: Array<{ title: string; lessons: YearPlanLesson[] }>, format: LessonExportFormat): { content: string; ext: string; mime: string } {
  if (format === 'txt') {
    let text = `${context.className} Lesson Plans\n${context.school} Â· ${context.level}\n${context.scope}\n${'='.repeat(50)}\n\n`;
    for (const group of groups) { text += `\n${group.title}\n${'-'.repeat(group.title.length)}\n`; for (const lesson of group.lessons) { text += lessonTextContent(lesson); } }
    text += `\n${'='.repeat(50)}\nGenerated by EduTrack\n`;
    return { content: text, ext: 'txt', mime: 'text/plain' };
  }
  if (format === 'md') {
    let md = `# ${context.className} Lesson Plans\n\n**${context.school}** Â· **${context.level}**\n*${context.scope}*\n\n---\n\n`;
    for (const group of groups) { md += `## ${group.title}\n\n`; for (const lesson of group.lessons) { md += lessonMdContent(lesson); md += `---\n\n`; } }
    md += `\n*Generated by EduTrack*\n`;
    return { content: md, ext: 'md', mime: 'text/markdown' };
  }
  if (format === 'docx') {
    return { content: buildLessonPlanExportHtmlPage(context, groups, false), ext: 'doc', mime: 'application/msword' };
  }
  if (format === 'pdf') {
    return { content: buildLessonPlanExportHtmlPage(context, groups, true), ext: 'html', mime: 'text/html' };
  }
  const html = buildLessonPlanExportHtmlBody(groups);
  const wrapped = professionalLessonTemplateFrame(context, html);
  return { content: wrapped, ext: 'html', mime: 'text/html' };
}

function buildLessonPlanExportHtmlPage(context: { school: string; className: string; level: string; scope: string }, groups: Array<{ title: string; lessons: YearPlanLesson[] }>, forPdf: boolean) {
  const body = buildLessonPlanExportHtmlBody(groups);
  const baseCss = `body{font-family:Inter,Arial,Helvetica,sans-serif;color:#0f172a;font-size:11pt;line-height:1.4}h1{font-size:24pt;margin:0 0 4pt;font-weight:800}h2{font-size:15pt;color:#0f766e;font-weight:800;margin:20pt 0 10pt;padding-bottom:3pt;border-bottom:2pt solid #eef2f6}.meta{color:#0f766e;font-weight:700;font-size:10pt;text-transform:uppercase;letter-spacing:0.3pt}.group-title{font-size:14pt;color:#0f766e;font-weight:800;margin:16pt 0 8pt;padding-bottom:2pt;border-bottom:1pt solid #d8e0ea}.lesson{border:1px solid #d8e0ea;border-radius:4pt;margin-bottom:12pt;page-break-inside:avoid}.lesson-header{background:#f8fafc;padding:8pt 12pt;border-bottom:1px solid #eef2f6}.lesson-header h3{font-size:13pt;margin:6pt 0 0;font-weight:800;color:#0f172a}.lesson-body{padding:6pt 12pt 10pt}.bd-section{margin:8pt 0 0}.bd-section h4{font-size:10.5pt;font-weight:800;color:#0f766e;text-transform:uppercase;letter-spacing:0.3pt;margin:0 0 3pt;padding-bottom:2pt;border-bottom:1pt solid #eef2f6}.bd-section p{margin:2pt 0;color:#1e293b}.bd-section ul{margin:2pt 0 4pt;padding-left:18pt}.bd-section li{margin:1pt 0;color:#334155}.bd-subsection{margin-top:6pt;padding:5pt 7pt;border:1pt solid #eef2f6;border-radius:4pt;background:#fbfdff}.bd-subsection h5{margin:0 0 2pt;font-size:9.5pt;color:#0f766e;text-transform:uppercase;letter-spacing:0.2pt}.bd-subsection p{margin:0}hr.section-divider{border:none;border-top:1pt solid #eef2f6;margin:6pt 0}.meta-table{width:100%;border-collapse:collapse;font-size:10pt;color:#475569}.meta-table td{width:33%;padding:1pt 0}.meta-table td strong{color:#0f172a;font-weight:700}.flow-table{width:100%;border-collapse:collapse;margin:4pt 0;font-size:10pt}.flow-table th{background:#f1f5f9;color:#0f172a;font-weight:800;text-align:left;padding:5pt 6pt;border:1px solid #d8e0ea;font-size:9.5pt;text-transform:uppercase;letter-spacing:0.2pt}.flow-table td{padding:5pt 6pt;border:1px solid #d8e0ea;vertical-align:top;color:#334155}.flow-table .flow-phase{font-weight:700;color:#0f172a;white-space:nowrap;width:22%}.flow-table .flow-time{white-space:nowrap;width:10%;text-align:center;font-weight:700;color:#64748b}.checkbox-list{list-style:none;padding-left:0}.checkbox-item{color:#334155;margin:2pt 0}.vocab-list{display:flex;flex-wrap:wrap;gap:3pt 8pt;list-style:none;padding-left:0}.vocab-list li{background:#f1f5f9;padding:1pt 6pt;border-radius:3pt;font-size:10pt;color:#0f172a;font-weight:600}`;
  const style = forPdf
    ? `@page{margin:0.5in}body{margin:0;${baseCss}}`
    : `body{margin:0.5in;${baseCss}}`;
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${escapeHtml(context.className)} Lesson Plans</title><style>${style}</style></head><body><div class="meta">${escapeHtml(context.school)} Â· ${escapeHtml(context.level)}</div><h1>${escapeHtml(context.className)} Lesson Plans</h1><p>${escapeHtml(context.scope)}</p>${body}</body></html>`;
}

function buildLessonPlanExportHtmlBody(groups: Array<{ title: string; lessons: YearPlanLesson[] }>): string {
  return groups.map((group) => `<section class="group"><h2 class="group-title">${escapeHtml(group.title)}</h2>${group.lessons.map((lesson) => renderLessonHtml(lesson)).join('')}</section>`).join('');
}

function renderLessonHtml(lesson: YearPlanLesson): string {
  const duration = selectedClass(state)?.periodMinutes ?? 45;
  const plan = parseLessonPlan(lesson.lessonObjective);
  const klass = selectedClass(state);
  const backwardDesignHtml = plan?.backwardDesign
    ? `<div class="bd-section"><h4>Backward Design Template</h4>${renderBackwardDesignHtml(plan.backwardDesign)}</div>`
    : '';
  if (!plan) {
    return `<article class="lesson"><div class="lesson-header"><table class="meta-table"><tr><td><strong>Date:</strong> ${escapeHtml(lesson.teachingDate)}</td><td><strong>Day:</strong> ${escapeHtml(capitalize(lesson.weekday))}</td><td><strong>Duration:</strong> ${duration} min</td></tr></table><h3>${escapeHtml(lesson.lessonTitle)}</h3></div><div class="lesson-body"><p><strong>Objective:</strong> ${escapeHtml(lesson.lessonObjective ?? 'To be added.')}</p></div></article>`;
  }
  return `<article class="lesson backward-design">
    <div class="lesson-header">
      <table class="meta-table">
        <tr><td><strong>Class:</strong> ${escapeHtml(klass?.name ?? '')}</td><td><strong>Unit:</strong> ${escapeHtml(lesson.lessonTitle)}</td><td><strong>Date:</strong> ${escapeHtml(lesson.teachingDate)}</td></tr>
        <tr><td><strong>Duration:</strong> ${duration} min</td><td><strong>Day:</strong> ${escapeHtml(capitalize(lesson.weekday))}</td><td><strong>Teacher:</strong> ${escapeHtml(state.teacherProfile.name)}</td></tr>
      </table>
    </div>
    <div class="lesson-body">
      <div class="bd-section"><h4>Learning Objective</h4><p>${escapeHtml(plan.objective)}</p></div>
      <div class="bd-section"><h4>Transfer Goal</h4><p>${escapeHtml(plan.transferGoal)}</p></div>
      <div class="bd-section"><h4>Enduring Understanding</h4><p>${escapeHtml(plan.enduringUnderstanding)}</p></div>
      <div class="bd-section"><h4>Essential Vocabulary</h4><ul class="vocab-list">${plan.vocabulary.map((v: string) => `<li>${escapeHtml(v)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Guiding Questions</h4><ul>${plan.essentialQuestions.map((q: string) => `<li>${escapeHtml(q)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Knowledge</h4><ul>${plan.knowledge.map((k: string) => `<li>${escapeHtml(k)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Skills</h4><ul>${plan.skills.map((s: string) => `<li>${escapeHtml(s)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Success Criteria</h4><ul class="checkbox-list">${plan.successCriteria.map((c: string) => `<li class="checkbox-item">â˜ ${escapeHtml(c)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Assessment Evidence</h4><p>${escapeHtml(plan.assessment)}</p>${plan.performanceTask ? `<p><strong>Performance Task:</strong> ${escapeHtml(plan.performanceTask)}</p>` : ''}</div>
      <div class="bd-section"><h4>Learning Plan</h4><table class="flow-table"><thead><tr><th>Stage</th><th>Time</th><th>Teacher / Student Activity</th></tr></thead><tbody>${plan.lessonFlow.map((step: { phase: string; time: string; description: string }) => `<tr><td class="flow-phase">${escapeHtml(step.phase)}</td><td class="flow-time">${escapeHtml(step.time)}</td><td>${escapeHtml(step.description)}</td></tr>`).join('')}</tbody></table></div>
      <div class="bd-section"><h4>Materials</h4><ul>${plan.materials.map((m: string) => `<li>${escapeHtml(m)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Differentiation</h4><p>${escapeHtml(plan.differentiation).replace(/\n/g, '<br>')}</p></div>
      ${plan.homework ? `<div class="bd-section"><h4>Homework / Follow-Up</h4><p>${escapeHtml(plan.homework)}</p></div>` : ''}
      ${plan.crossCurricular ? `<div class="bd-section"><h4>Cross-Curricular Connections</h4><p>${escapeHtml(plan.crossCurricular)}</p></div>` : ''}
      ${backwardDesignHtml}
    </div></article>`;
}

function professionalLessonTemplateFrame(context: { school: string; className: string; level: string; scope: string }, bodyHtml: string): string {
  const notes = state.lessonContextNotes.trim();
  const notesSection = notes ? `<div class="notes-section"><h4>Teacher context notes</h4><p>${escapeHtml(notes).replace(/\n/g, '<br>')}</p></div>` : '';
  const klass = selectedClass(state);
  const duration = klass?.periodMinutes ?? 45;
  return `<!doctype html><html><head><meta charset="utf-8" /><title>${escapeHtml(context.className)} Lesson Plans</title>
<style>*,*::before,*::after{box-sizing:border-box}
body{font-family:Inter,-apple-system,BlinkMacSystemFont,'Segoe UI',Arial,Helvetica,sans-serif;color:#0f172a;margin:0;background:#f1f5f9;font-size:13px;line-height:1.5}
.cover{max-width:900px;margin:32px auto 0;background:#fff;border:1px solid #d8e0ea;border-radius:10px;padding:32px 36px}
.cover h1{font-size:30px;margin:4px 0 6px;font-weight:800;color:#0f172a}.cover p{margin:0;color:#64748b}
.meta{color:#0f766e;font-weight:700;font-size:13px;text-transform:uppercase;letter-spacing:0.3px}
.group{max-width:900px;margin:20px auto}.group-title{font-size:20px;color:#0f766e;margin:0 0 12px 4px;font-weight:800;padding-bottom:4px;border-bottom:2px solid #eef2f6}
.lesson{background:#fff;border:1px solid #d8e0ea;border-radius:8px;margin-bottom:14px;overflow:hidden}
.lesson-header{background:#f8fafc;padding:12px 18px;border-bottom:1px solid #eef2f6}
.lesson-header h3{font-size:16px;margin:8px 0 0;font-weight:800;color:#0f172a}
.lesson-body{padding:10px 18px 16px}.bd-section{margin:12px 0 0}
.bd-section h4{font-size:12px;font-weight:800;color:#0f766e;text-transform:uppercase;letter-spacing:0.3px;margin:0 0 4px;padding-bottom:3px;border-bottom:2px solid #eef2f6}
.bd-section p{margin:3px 0;color:#1e293b}.bd-section ul{margin:2px 0 6px;padding-left:18px}
.bd-section li{margin:2px 0;color:#334155}.bd-subsection{margin-top:8px;padding:7px 9px;border:1px solid #e2e8f0;border-radius:6px;background:#fbfdff}
.bd-subsection h5{margin:0 0 3px;font-size:11px;color:#0f766e;text-transform:uppercase;letter-spacing:0.25px}.bd-subsection p{margin:0}
.meta-table{width:100%;border-collapse:collapse;font-size:11px;color:#475569}.meta-table td{width:33%;padding:2px 0}.meta-table td strong{color:#0f172a;font-weight:700}
.flow-table{width:100%;border-collapse:collapse;margin:4px 0;font-size:12px}.flow-table th{background:#f1f5f9;color:#0f172a;font-weight:800;text-align:left;padding:6px 8px;border:1px solid #d8e0ea;font-size:10px;text-transform:uppercase;letter-spacing:0.2px}
.flow-table td{padding:6px 8px;border:1px solid #d8e0ea;vertical-align:top;color:#334155}.flow-table .flow-phase{font-weight:700;color:#0f172a;white-space:nowrap;width:22%}
.flow-table .flow-time{white-space:nowrap;width:10%;text-align:center;font-weight:700;color:#64748b}
.checkbox-list{list-style:none;padding-left:0}.checkbox-item{color:#334155;margin:3px 0}
.vocab-list{display:flex;flex-wrap:wrap;gap:4px 10px;list-style:none;padding-left:0}
.vocab-list li{background:#f1f5f9;padding:2px 8px;border-radius:4px;font-size:12px;color:#0f172a;font-weight:600}
.notes-section{margin-top:16px;padding:14px;background:#f8fafc;border-radius:6px;border-left:3px solid #0f766e}
.notes-section h4{font-size:12px;font-weight:800;color:#0f766e;text-transform:uppercase;margin:0 0 6px}
.notes-section p{margin:0;color:#475569;font-size:13px;line-height:1.6;white-space:pre-wrap}
@media print{body{background:#fff}.cover,.lesson{box-shadow:none;border-color:#ccc}.lesson{break-inside:avoid}}
</style></head><body>
<section class="cover"><div class="meta">${escapeHtml(context.school)} Â· ${escapeHtml(context.level)}</div><h1>${escapeHtml(context.className)} Lesson Plans</h1><p>${escapeHtml(context.scope)} Â· ${duration} min lessons Â· Generated by EduTrack</p>${notesSection}</section>
${bodyHtml}</body></html>`;
}

// ==================== AGENTS PAGE ====================

function renderAgents() {
  const grouped = groupAgentInsights();
  return `
    <section class="agent-toolbar animate-fade-in-up delay-100">
      <div><p class="eyebrow">AI Workers</p><h2>Intelligent agents</h2><p>Agents run automatically when you select a class. They analyse attendance, pacing, performance, assignments, engagement, and reports.</p></div>
      <button id="run-agents" class="primary-button" type="button" ${selectedClass(state) ? '' : 'disabled'}>${icon('cpu')} Run agents now</button>
    </section>
    <section class="agent-grid">${agentCards.map((agent, index) => {
      const agentInsights = grouped[agent.type] ?? [];
      const activeCount = agentInsights.length;
      const maxSeverity = agentInsights.reduce((max, i) => i.severity === 'high' ? 'high' : max, 'neutral');
      const cardTone = maxSeverity === 'high' ? 'amber' : maxSeverity === 'warning' ? agent.color : 'idle';
      return `<article class="agent-card ${agent.color} animate-fade-in-up" style="animation-delay: ${(index * 50) + 200}ms"><div class="agent-icon">${icon(agent.icon)}</div><label class="switch"><input type="checkbox" data-agent-toggle="${agent.type}" checked /><span></span></label><h2>${agent.title}</h2><p>${agent.body}</p>${activeCount > 0 ? `<span class="agent-badge ${cardTone}">${activeCount} signal${activeCount === 1 ? '' : 's'}</span>` : '<span class="agent-badge idle">No signals</span>'}</article>`;
    }).join('')}</section>
    ${state.insights.length > 0 ? `<div class="panel animate-fade-in-up delay-400" style="padding:0;overflow:hidden"><div class="syllabus-review-header" style="padding:14px 18px"><p class="eyebrow">Agent insights</p><span>${state.insights.length} total</span></div><div class="agent-insight-list">${state.insights.map((insight) => `<div class="agent-insight-row ${insight.severity}"><span class="agent-insight-severity ${insight.severity}">${capitalize(insight.severity)}</span><div class="agent-insight-body"><strong>${escapeHtml(insight.title)}</strong><p>${escapeHtml(insight.body)}</p></div><span class="agent-insight-type">${capitalize(insight.agentType)}</span></div>`).join('')}</div></div>` : `<div class="panel animate-fade-in-up delay-400"><div class="panel-head"><h2>Recent insights</h2></div>${emptyState('No insights yet', 'Select a class to run agents automatically. Insights appear here when agents detect patterns.')}</div>`}
  `;
}

function groupAgentInsights(): Record<string, typeof state.insights> {
  const groups: Record<string, typeof state.insights> = {};
  for (const insight of state.insights) { (groups[insight.agentType] ??= []).push(insight); }
  return groups;
}

// ==================== TESTS PAGE ====================

function renderTests() {
  const activeClass = selectedClass(state);
  const hasClass = Boolean(activeClass);
  const selectedTest = state.tests.find((t) => t.id === state.currentTestId);
  return `
    <section class="tests-layout">
      <article class="panel tests-create-panel">
        <p class="eyebrow">Tests</p><h2>Create a new test</h2><p>Design a test for your class. Add instructions for the LLM, scoring rules, then collect student submissions.</p>
        <form id="test-create-form" class="tests-create-form">
          <div class="tests-create-actions">
            <button id="test-generate-syllabus" class="ghost-button" type="button" ${hasClass ? '' : 'disabled'}>${icon('book')} Create from syllabus</button>
            <span class="input-hint">Auto-fills detailed test instructions and rubric from this level's syllabus units.</span>
          </div>
          <label><span class="label">Test title</span><input id="test-title" type="text" placeholder="e.g. Unit 3 Algebra Quiz" ${hasClass ? '' : 'disabled'} /></label>
          <label><span class="label">Instructions for assessment</span><textarea id="test-instructions" rows="4" placeholder="Describe what the test covers..." ${hasClass ? '' : 'disabled'}></textarea></label>
          <label><span class="label">Scoring rubric</span><textarea id="test-rubric" rows="3" placeholder="Define the scoring criteria..." ${hasClass ? '' : 'disabled'}></textarea></label>
          <label><span class="label">Maximum score</span><input id="test-max-score" type="number" min="1" max="1000" value="100" ${hasClass ? '' : 'disabled'} /></label>
          <button class="primary-button" type="submit" ${hasClass ? '' : 'disabled'}>${icon('check-circle')} Create test</button>
        </form>
      </article>
      <article class="panel"><div class="panel-head"><p class="eyebrow">Existing tests</p><h2>${activeClass ? escapeHtml(activeClass.name) : 'No class'}</h2></div>${state.tests.length === 0 ? emptyState('No tests yet', 'Create a test using the form. Once created, you can collect submissions and score them.') : `<div class="tests-list">${state.tests.map((test) => `<div class="test-row ${test.id === state.currentTestId ? 'active' : ''}" data-select-test="${escapeHtml(test.id)}"><div class="test-row-info"><strong>${escapeHtml(test.title)}</strong><span>${escapeHtml(test.status)} Â· ${escapeHtml(test.createdAt.slice(0, 10))}</span></div><span class="test-row-status ${test.status}">${test.status === 'completed' ? 'Completed' : 'Pending'}</span></div>`).join('')}</div>`}</article>
      ${selectedTest ? `
        <article class="panel tests-submissions-panel">
          <div class="panel-head"><div><p class="eyebrow">Submissions</p><h2>${escapeHtml(selectedTest.title)}</h2></div><div class="tests-actions"><button id="auto-score-test" class="primary-button" type="button">${icon('cpu')} Auto-score all</button><button id="export-test-results" class="ghost-button" type="button">${icon('upload')} Export</button></div></div>
          <p>${escapeHtml(selectedTest.instructions)}</p>
          <div class="tests-max-score"><span>Max score:</span> <strong>${selectedTest.maxScore}</strong></div>
          <p class="input-hint">Upload PDF, DOC/DOCX, TXT/MD/CSV/JSON/XML/HTML, image, or audio. Image OCR uses local <code>tesseract</code>; audio transcription uses local <code>whisper</code>. DOC/DOCX uses <code>pandoc</code> when available.</p>
          ${state.testSubmissions.length === 0 ? emptyState('No submissions loaded', 'Select a test above to load submissions.') : `<div class="tests-submissions-grid">${state.testSubmissions.map((sub) => `<div class="submission-card"><div class="submission-head"><strong>${escapeHtml(sub.studentName)}</strong><span class="submission-status ${sub.status}">${sub.status === 'scored' ? 'Scored' : 'Pending'}</span></div><div class="submission-files">${sub.fileName ? `<span class="submission-file">${escapeHtml(sub.fileName)} <button type="button" class="compact-button ghost-button" data-view-file="${escapeHtml(sub.attachmentId ?? '')}">View</button></span>` : '<span class="submission-file empty">No file submitted</span>'}</div><label class="submission-score-field"><span class="label">Score</span><input type="number" min="0" max="${selectedTest.maxScore}" step="0.5" value="${sub.scoreValue ?? ''}" data-score-student="${escapeHtml(sub.studentId)}" placeholder="-" /></label><label class="submission-comment-field"><span class="label">Feedback</span><textarea data-comment-student="${escapeHtml(sub.studentId)}" rows="2" placeholder="Add feedback">${escapeHtml(sub.teacherComment ?? '')}</textarea></label><label class="submission-upload-field"><span class="label">Upload file</span><input type="file" data-upload-student="${escapeHtml(sub.studentId)}" accept=".pdf,.doc,.docx,.txt,.md,.csv,.json,.xml,.html,.htm,image/*,audio/*" /></label><button type="button" class="ghost-button compact-button" data-save-score="${escapeHtml(sub.studentId)}">Save</button></div>`).join('')}</div>`}
        </article>` : ''}
    </section>
  `;
}

// ==================== REPORTS PAGE ====================

function renderReports() {
  const activeClass = selectedClass(state);
  const hasClass = Boolean(activeClass);
  return `
    <section class="reports-layout">
      <article class="panel">
        <p class="eyebrow">Student reports</p><h2>Generate reports for ${escapeHtml(activeClass?.name ?? 'a class')}</h2><p>Reports include attendance data, assessment scores, and performance trends. Select a class and add any specific instructions for the report content.</p>
        <label style="display:flex;flex-direction:column;gap:4px;margin:var(--space-3) 0"><span class="label">Instructions for report generation</span><textarea id="report-instructions" rows="3" style="font-family:inherit;font-size:13px;resize:vertical" placeholder="Optional: add focus areas, tone preferences, or specific points to include...">${escapeHtml(state.reportInstructions)}</textarea></label>
        <label style="display:flex;align-items:center;gap:var(--space-2);margin-bottom:var(--space-3)"><span class="label" style="white-space:nowrap">Word count:</span><input id="report-word-count" type="number" min="80" max="2000" value="${state.reportWordCount}" style="width:80px;min-height:0;padding:4px 8px;font-size:14px" /></label>
        <button id="generate-all-reports" class="primary-button" type="button" ${hasClass && state.students.length > 0 ? '' : 'disabled'}>${icon('book')} Generate reports for all students</button>
      </article>
      ${hasClass ? `<div class="panel" style="padding:0;overflow:hidden;grid-column:1/-1"><div class="syllabus-review-header" style="padding:14px 18px"><p class="eyebrow">Students</p><span>${state.students.length} total</span></div><div class="reports-grid">${state.students.map((student) => { const status = state.reportStatuses.find((r) => r.studentId === student.id); const hasReport = status?.reportExists ?? false; return `<div class="report-student-card"><div class="report-student-head"><strong>${escapeHtml(student.fullName)}</strong>${hasReport ? '<span class="test-row-status completed">Done</span>' : '<span class="test-row-status pending">No report</span>'}</div>${hasReport ? `<div class="report-actions"><button type="button" class="ghost-button compact-button" data-view-report="${escapeHtml(status!.reportId!)}">View</button><button type="button" class="ghost-button compact-button" data-export-report="${escapeHtml(status!.reportId!)}">${icon('upload')} Export</button></div>` : ''}</div>`; }).join('')}</div></div>` : ''}
    </section>
  `;
}

// ==================== SETTINGS PAGE ====================

function renderSettings() {
  const config = state.llmConfig;
  const diagnostics = state.gradingDiagnostics;
  const installableKeys = new Set(['opencode', 'tesseract', 'whisper', 'pandoc']);
  const missingInstallableTools = diagnostics
    ? diagnostics.tools.filter((tool) => !tool.available && installableKeys.has(tool.key))
    : [];
  const providerName = config?.provider === 'opencode' ? 'OpenCode' : 'Ollama';
  const hasModels = state.llmModels.length > 0;
  const statusIcon = config?.available || hasModels ? 'check-circle' : 'alert-circle';
  const statusClass = config?.available ? 'notice-ok' : hasModels ? 'notice-info' : 'notice-warn';
  const statusText = llmStatusText(config, providerName, hasModels);
  return `
    <section class="settings-grid">
      <article class="panel animate-fade-in-up delay-100">
        <p class="eyebrow">Teacher profile</p><h2>Your workspace identity</h2>
        <form id="teacher-profile-form" class="teacher-profile-form">
          <div class="teacher-profile-preview">${teacherAvatar('profile-avatar')}<div><strong>${escapeHtml(state.teacherProfile.name)}</strong><span>${escapeHtml(state.teacherProfile.title)}</span></div></div>
          <label><span class="label">Name</span><input id="teacher-name" type="text" value="${escapeHtml(state.teacherProfile.name)}" placeholder="Teacher name" required /></label>
          <label><span class="label">Role / title</span><input id="teacher-title" type="text" value="${escapeHtml(state.teacherProfile.title)}" placeholder="Teacher workspace" /></label>
          <label class="teacher-image-field"><span class="label">Profile image</span><input id="teacher-image" type="file" accept="image/png,image/jpeg,image/webp,image/svg+xml" /><span class="input-hint">Optional image shown in the sidebar profile card.</span></label>
          <div id="teacher-image-preview" class="school-logo-preview" aria-live="polite">${state.teacherProfile.image ? `<img src="${escapeHtml(state.teacherProfile.image)}" alt="Current teacher profile image" /><span>Current image</span>` : '<span>No image selected</span>'}</div>
          <button class="primary-button" type="submit">${icon('users')} Save profile</button>
        </form>
      </article>
      <article class="panel animate-fade-in-up delay-100">
        <p class="eyebrow">Language model</p><h2>Local model settings</h2>
        <form id="provider-form" class="settings-form">
          <label><span class="label">Provider</span><select id="provider-select"><option value="ollama" ${config?.provider === 'ollama' ? 'selected' : ''}>Ollama</option><option value="opencode" ${config?.provider === 'opencode' ? 'selected' : ''}>OpenCode</option></select></label>
          <label><span class="label">Model</span><select id="model-select" ${state.llmModels.length === 0 ? 'disabled' : ''}>${state.llmModels.length === 0 ? '<option value="">No models found</option>' : state.llmModels.map((m) => `<option value="${escapeHtml(m.id)}" ${config?.model === m.id ? 'selected' : ''}>${escapeHtml(m.label)}</option>`).join('')}</select></label>
          <button class="primary-button" type="submit" ${state.llmModels.length === 0 ? 'disabled' : ''}>Save</button>
        </form>
        <div class="notice ${statusClass}">${icon(statusIcon)}<span>${statusText}</span></div>
        ${config?.detail && !config.available ? `<p class="settings-detail">${escapeHtml(config.detail)}</p>` : ''}
      </article>
      <article class="panel animate-fade-in-up delay-200">
        <div class="panel-head">
          <div><p class="eyebrow">Diagnostics</p><h2>Grading tools and storage</h2></div>
          <div style="display:flex;gap:8px;flex-wrap:wrap">
            <button id="install-grading-tools" class="primary-button" type="button" ${state.gradingToolsInstallBusy || missingInstallableTools.length === 0 ? 'disabled' : ''}>${state.gradingToolsInstallBusy ? '<span class="spinner"></span>' : icon('upload')} ${state.gradingToolsInstallBusy ? 'Installing...' : 'Install missing tools'}</button>
            <button id="refresh-diagnostics" class="ghost-button" type="button" ${state.gradingToolsInstallBusy ? 'disabled' : ''}>${icon('cpu')} Run checks</button>
          </div>
        </div>
        ${diagnostics ? `
          <div class="notice ${diagnostics.llmAvailable ? 'notice-ok' : 'notice-warn'}">
            ${icon(diagnostics.llmAvailable ? 'check-circle' : 'alert-circle')}
            <span>LLM runtime check: ${escapeHtml(diagnostics.llmProvider)} / ${escapeHtml(diagnostics.llmModel || 'no model selected')} ${diagnostics.llmAvailable ? 'CLI detected' : 'CLI unavailable'}</span>
          </div>
          ${diagnostics.llmDetail ? `<p class="settings-detail">${escapeHtml(diagnostics.llmDetail)}</p>` : ''}
          <p class="settings-detail">Note: CLI/model checks do not verify provider billing/quota. Billing errors only appear when running a real AI request.</p>
          <div class="diag-grid">${diagnostics.tools.map((tool) => {
            const canInstall = installableKeys.has(tool.key);
            const installButton = !tool.available && canInstall
              ? `<button class="ghost-button compact-button" type="button" data-install-tool="${escapeHtml(tool.key)}" data-install-tool-label="${escapeHtml(tool.label)}" ${state.gradingToolsInstallBusy ? 'disabled' : ''}>Install</button>`
              : '';
            const installNote = state.gradingToolInstallNotes[tool.key] ?? '';
            const installNoteHtml = installNote ? `<p class="settings-detail" style="margin-top:2px;color:var(--ink-strong)"><strong>Installer:</strong> ${escapeHtml(installNote)}</p>` : '';
            return `<div class="diag-row"><strong>${escapeHtml(tool.label)}</strong><div style="display:flex;align-items:center;gap:8px;justify-content:flex-end"><span class="test-row-status ${tool.available ? 'completed' : 'pending'}">${tool.available ? 'Available' : 'Missing'}</span>${installButton}</div><p>${escapeHtml(summarizeStartupToolDetail(tool.detail, tool.key))}</p>${installNoteHtml}</div>`;
          }).join('')}</div>
          <div class="diag-storage">
            <p><strong>Storage root:</strong> ${escapeHtml(diagnostics.storageRoot)}</p>
            <p><strong>Storage pattern:</strong> ${escapeHtml(diagnostics.storagePattern)}</p>
          </div>
          <p class="settings-detail">Installer covers: <code>opencode</code>, <code>tesseract</code>, <code>whisper</code>, <code>pandoc</code>. Legacy <code>antiword</code>/<code>catdoc</code> remain optional.</p>
        ` : `<div class="notice notice-info">${icon('alert-circle')}<span>Diagnostics not loaded yet. Click Run checks.</span></div>`}
      </article>
      <article class="panel animate-fade-in-up delay-300">
        <p class="eyebrow">Data philosophy</p><h2>No synthetic data</h2><p>EduTrack displays honest empty states when local records don't exist. No demo metrics or fallback data.</p>
      </article>
      <article class="panel animate-fade-in-up delay-400">
        <p class="eyebrow">Danger zone</p><h2>Clear school history</h2><p>Permanently delete all data (levels, classes, students, attendance, assessments, reports) for a selected school. This cannot be undone.</p>
        <div style="display:flex;align-items:center;gap:var(--space-3);flex-wrap:wrap"><select id="clear-school-select" style="flex:1;min-width:180px"><option value="">Select a school</option>${state.schools.map((s) => `<option value="${escapeHtml(s.id)}">${escapeHtml(s.name)}</option>`).join('')}</select><button id="clear-school-btn" class="danger-button" type="button" disabled>Clear all data</button></div>
      </article>
    </section>
  `;
}

function llmStatusText(config: LlmConfig | null, providerName: string, hasModels: boolean) {
  if (!config) return 'No provider configured.';
  const modelCount = `${state.llmModels.length} ${state.llmModels.length === 1 ? 'model' : 'models'}`;
  if (config.available) return `${providerName} CLI detected - ${modelCount} listed (billing/quota checked at request time)`;
  if (hasModels) return `${providerName} CLI not detected - showing ${modelCount} listed models`;
  return `${providerName} unavailable - no models found`;
}

async function refreshGradingDiagnostics() {
  const diagnosticsCall = invoke<GradingDiagnostics>('get_grading_diagnostics');
  state.gradingDiagnostics = diagnosticsCall ? await diagnosticsCall.catch(() => null) : null;
}

async function installMissingGradingTools() {
  state.gradingToolsInstallBusy = true;
  render();
  try {
    const diagnostics = state.gradingDiagnostics;
    const installableKeys = new Set(['opencode', 'tesseract', 'whisper', 'pandoc']);
    const requested = diagnostics
      ? diagnostics.tools.filter((tool) => !tool.available && installableKeys.has(tool.key)).map((tool) => tool.key)
      : ['opencode', 'tesseract', 'whisper', 'pandoc'];
    const result = await invoke<GradingToolInstallResult>('install_grading_tools', { toolKeys: requested });
    for (const key of requested) {
      if (result?.installed?.includes(key)) {
        state.gradingToolInstallNotes[key] = 'Installed successfully.';
      } else if (result?.alreadyAvailable?.includes(key)) {
        state.gradingToolInstallNotes[key] = 'Already available.';
      } else {
        const failed = result?.failed?.find((item) => item.key === key);
        const note = (result?.notes ?? []).find((line) => line.toLowerCase().includes(key.toLowerCase()));
        state.gradingToolInstallNotes[key] = summarizeStartupToolDetail(note || failed?.detail || 'Install attempt failed.', key);
      }
    }
    await refreshGradingDiagnostics();
    render();
    const installed = result?.installed?.length ?? 0;
    const already = result?.alreadyAvailable?.length ?? 0;
    const failed = result?.failed?.length ?? 0;
    showToast(`Tool install finished: ${installed} installed, ${already} already available, ${failed} failed.`);
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error));
  } finally {
    state.gradingToolsInstallBusy = false;
    render();
  }
}

async function installSingleGradingTool(toolKey: string, label: string) {
  if (!toolKey) return;
  state.gradingToolsInstallBusy = true;
  render();
  try {
    const result = await invoke<GradingToolInstallResult>('install_grading_tools', { toolKeys: [toolKey] });
    await refreshGradingDiagnostics();
    render();
    const installed = result?.installed?.includes(toolKey);
    const already = result?.alreadyAvailable?.includes(toolKey);
    const failed = result?.failed?.find((item) => item.key === toolKey);
    const installNotes = (result?.notes ?? []).filter((line) => line.toLowerCase().includes(toolKey.toLowerCase()) || line.toLowerCase().includes(label.toLowerCase()));
    const notePreview = installNotes.slice(0, 1).join(' ');
    if (installed) {
      state.gradingToolInstallNotes[toolKey] = 'Installed successfully.';
      showToast(`${label} installed successfully.`);
      return;
    }
    if (already) {
      state.gradingToolInstallNotes[toolKey] = 'Already available.';
      showToast(`${label} is already available.`);
      return;
    }
    if (failed) {
      const detail = summarizeStartupToolDetail(notePreview || failed.detail, toolKey);
      state.gradingToolInstallNotes[toolKey] = detail;
      showToast(`${label} install failed: ${detail}`);
      return;
    }
    state.gradingToolInstallNotes[toolKey] = 'Install completed. Re-run checks to confirm status.';
    showToast(`${label} install finished. Run checks to confirm status.`);
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error));
  } finally {
    state.gradingToolsInstallBusy = false;
    render();
  }
}

async function markLessonStatus(lessonId: string, status: string) {
  if (!lessonId || !status) return;
  await invoke('update_lesson_status', { lessonId, status });
  for (const lesson of state.generatedLessons) {
    if (lesson.id === lessonId) lesson.status = status;
  }
  for (const entry of state.allCalendarLessons) {
    if (entry.lesson.id === lessonId) entry.lesson.status = status;
  }
}

function bindLessonStatusButtons(scope: ParentNode) {
  scope.querySelectorAll<HTMLButtonElement>('[data-mark-lesson]').forEach((button) => {
    if (button.dataset.lessonStatusBound === '1') return;
    button.dataset.lessonStatusBound = '1';
    button.addEventListener('click', async (event) => {
      event.preventDefault();
      event.stopPropagation();
      const lessonId = button.dataset.markLesson ?? '';
      const status = button.dataset.status ?? '';
      try {
        await markLessonStatus(lessonId, status);
        render();
        showToast(`Lesson marked as ${status}`);
      } catch (error) {
        showToast(String(error));
      }
    });
  });
}

// ==================== EVENT BINDING ====================

function bindShellEvents() {
  document.querySelectorAll<HTMLElement>('[data-page]').forEach((el) => {
    el.addEventListener('click', async (e) => {
      e.preventDefault();
      if (state.lessonPlansBusy) {
        const proceed = await openConfirmModal({
          id: 'confirm-leave-generation',
          title: 'Generation In Progress',
          message: 'Detailed lesson plan generation is in progress. Leave this page?',
          confirmLabel: 'Leave page',
          cancelLabel: 'Stay here',
          confirmClass: 'danger',
        });
        if (!proceed) return;
      }
      state.page = el.dataset.page as Page;
      if (state.page === 'calendar') {
        await loadAllCalendarLessons(state);
        await loadAllCalendarSessions(state);
      }
      render();
    });
  });

  document.querySelectorAll<HTMLSelectElement>('[data-context]').forEach((select) => {
    select.addEventListener('change', async () => {
      const key = select.dataset.context as 'selectedSchoolId' | 'selectedLevelId' | 'selectedClassId';
      state[key] = select.value;
      if (key === 'selectedSchoolId' || key === 'selectedLevelId') {
        await loadHierarchy(state);
      } else {
        await loadClassData(state);
      }
      if (state.classTab === 'attendance' || state.classTab === 'calendar') {
        await ensureAttendanceSessionsForSelectedClass(state);
      }
      applySyllabusPlanningStateForCurrentContext();
      persistSyllabusPlanningState();
      render();
    });
  });
}

function bindPageEvents() {
  bindCountryPicker();

  document.querySelectorAll<HTMLButtonElement>('[data-open-class]').forEach((button) => {
    button.addEventListener('click', async () => {
      state.selectedClassId = button.dataset.openClass ?? state.selectedClassId;
      await loadClassData(state);
      if (state.classTab === 'attendance' || state.classTab === 'calendar') {
        await ensureAttendanceSessionsForSelectedClass(state);
      }
      applySyllabusPlanningStateForCurrentContext();
      persistSyllabusPlanningState();
      render();
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-remove-class]').forEach((button) => {
    button.addEventListener('click', async () => {
      const classId = button.dataset.removeClass ?? '';
      if (!state.backend) { showToast('Open the desktop app to remove classes'); return; }
      if (!classId) return;
      const klass = state.classes.find((item) => item.id === classId);
      if (klass) {
        const confirmed = await openConfirmModal({
          id: 'confirm-remove-class',
          title: `Remove ${klass.name}?`,
          message: 'Students and class records will be hidden with this class.',
          confirmLabel: 'Remove class',
          cancelLabel: 'Cancel',
          confirmClass: 'danger',
        });
        if (!confirmed) return;
      }
      await invoke('archive_class', { classId });
      if (state.selectedClassId === classId) state.selectedClassId = '';
      await loadHierarchy(state);
      await loadSchoolDirectory(state);
      render();
      showToast('Class removed');
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-remove-school]').forEach((button) => {
    button.addEventListener('click', async () => {
      const schoolId = button.dataset.removeSchool ?? '';
      if (!state.backend) { showToast('Open the desktop app to remove schools'); return; }
      if (!schoolId) return;
      const school = state.schools.find((item) => item.id === schoolId);
      if (school) {
        const confirmed = await openConfirmModal({
          id: 'confirm-remove-school',
          title: `Remove ${school.name}?`,
          message: 'Levels, classes, students, and school records will be hidden with this school.',
          confirmLabel: 'Remove school',
          cancelLabel: 'Cancel',
          confirmClass: 'danger',
        });
        if (!confirmed) return;
      }
      await invoke('archive_school', { schoolId });
      state.schools = (await invoke<School[]>('get_schools')) ?? [];
      await loadSchoolImages(state);
      if (state.selectedSchoolId === schoolId) { state.selectedSchoolId = state.schools[0]?.id ?? ''; state.selectedLevelId = ''; state.selectedClassId = ''; }
      await loadHierarchy(state);
      await loadSchoolDirectory(state);
      render();
      showToast('School removed');
    });
  });

  document.querySelector<HTMLFormElement>('#student-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const klass = selectedClass(state);
    const fullName = document.querySelector<HTMLInputElement>('#student-full-name')?.value.trim() ?? '';
    const preferredName = document.querySelector<HTMLInputElement>('#student-preferred-name')?.value.trim() || null;
    const studentCode = document.querySelector<HTMLInputElement>('#student-code')?.value.trim() || null;
    const ageRaw = document.querySelector<HTMLInputElement>('#student-age')?.value;
    const age = ageRaw ? parseInt(ageRaw, 10) : null;
    const gender = document.querySelector<HTMLSelectElement>('#student-gender')?.value || null;
    if (!state.backend) { showToast('Open the desktop app to save students'); return; }
    if (!klass || !fullName) return;
    await invoke<string>('create_student', { student: { id: '', classId: klass.id, schoolId: klass.schoolId, levelId: klass.levelId, fullName, preferredName, studentCode, age, gender, avatarSeed: null, notesPrivate: null, createdAt: '', updatedAt: '', archivedAt: null } });
    await loadClassData(state);
    await loadSchoolDirectory(state);
    render();
    showToast('Student added');
  });

  document.querySelectorAll<HTMLButtonElement>('[data-remove-student]').forEach((button) => {
    button.addEventListener('click', async () => {
      const studentId = button.dataset.removeStudent ?? '';
      if (!state.backend) { showToast('Open the desktop app to remove students'); return; }
      if (!studentId) return;
      await invoke('archive_student', { studentId });
      await loadClassData(state);
      render();
      showToast('Student removed');
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-save-note]').forEach((button) => {
    button.addEventListener('click', async () => {
      const studentId = button.dataset.saveNote ?? '';
      if (!state.backend || !studentId) return;
      const textarea = document.querySelector<HTMLTextAreaElement>(`[data-qa-note="${studentId}"]`);
      const note = textarea?.value?.trim() || null;
      try {
        await invoke('update_student_note', { studentId, note });
        const student = state.students.find((s) => s.id === studentId);
        if (student) student.notesPrivate = note;
        showToast('Note saved');
      } catch (error) { showToast(String(error)); }
    });
  });

  document.querySelector<HTMLInputElement>('#school-logo')?.addEventListener('change', async (event) => {
    const input = event.currentTarget as HTMLInputElement;
    const preview = document.querySelector<HTMLDivElement>('#school-logo-preview');
    const file = input.files?.[0];
    if (!preview || !file) { if (preview) preview.innerHTML = '<span>No image selected</span>'; return; }
    try {
      const dataUrl = await readImageAsDataUrl(file);
      preview.innerHTML = `<img src="${escapeHtml(dataUrl)}" alt="Selected school image preview" /><span>${escapeHtml(file.name)}</span>`;
    } catch (error) {
      input.value = '';
      preview.innerHTML = '<span>No image selected</span>';
      showToast(error instanceof Error ? error.message : String(error));
    }
  });

  document.querySelector<HTMLFormElement>('#school-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const name = document.querySelector<HTMLInputElement>('#school-name')?.value.trim() ?? '';
    if (!state.backend) { showToast('Open the desktop app to save schools'); return; }
    if (!name) return;
    const region = document.querySelector<HTMLInputElement>('#school-region')?.value || null;
    if (!region) { showToast('Select the school country so holidays can be automated'); return; }
    const yearLabel = document.querySelector<HTMLSelectElement>('#school-year')?.value || academicYearWindow().label;
    const logoFile = document.querySelector<HTMLInputElement>('#school-logo')?.files?.[0] ?? null;
    let logoPath: string | null = null;
    if (logoFile) {
      try {
        const buffer = await logoFile.arrayBuffer();
        const fileData = Array.from(new Uint8Array(buffer));
        logoPath = await invoke<string>('save_image_file', { fileData, fileName: logoFile.name });
      } catch (error) { showToast(error instanceof Error ? error.message : String(error)); return; }
    }
    state.setupBusy = 'school';
    state.setupNotice = 'Adding school...';
    render();
    const schoolId = await invoke<string>('create_school', { school: { id: '', name, logoPath, countryCode: null, regionCode: region, timezone: browserTimezone(), academicYearLabel: yearLabel, defaultReportWordCount: 200, createdAt: '', updatedAt: '', archivedAt: null } });
    state.schools = (await invoke<School[]>('get_schools')) ?? [];
    await loadSchoolImages(state);
    state.selectedSchoolId = schoolId ?? state.selectedSchoolId;
    state.selectedLevelId = '';
    state.selectedClassId = '';
    await loadHierarchy(state);
    await loadSchoolDirectory(state);
    state.setupBusy = '';
    state.setupNotice = 'School added. Now add a class for that school.';
    render();
    showToast('School added');
  });

  document.querySelector<HTMLFormElement>('#class-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const schoolId = document.querySelector<HTMLSelectElement>('#class-school')?.value ?? state.selectedSchoolId;
    const levelName = document.querySelector<HTMLInputElement>('#class-level')?.value.trim() || selectedLevel(state)?.name || 'General';
    const className = document.querySelector<HTMLInputElement>('#class-name')?.value.trim() ?? '';
    const subjectName = document.querySelector<HTMLInputElement>('#class-subject')?.value.trim() || null;
    const classStartDate = document.querySelector<HTMLInputElement>('#class-start-date')?.value || academicYearWindow().start;
    const classEndDate = document.querySelector<HTMLInputElement>('#class-end-date')?.value || academicYearWindow().end;
    const selectedWeekdays = Array.from(document.querySelectorAll<HTMLInputElement>('input[name="class-weekday"]:checked')).map((input) => input.value);
    if (!state.backend) { showToast('Open the desktop app to save classes'); return; }
    if (!schoolId || !className) return;
    if (selectedWeekdays.length === 0) { showToast('Select at least one class day'); return; }
    if (!parseYmdUtc(classStartDate) || !parseYmdUtc(classEndDate) || classStartDate > classEndDate) { showToast('Choose valid class start and finish dates'); return; }

    state.setupBusy = 'class';
    state.setupNotice = `Adding ${className} and preparing attendance dates...`;
    render();

    try {
      const year = academicYearWindow();
      let levelId = findLevelId(schoolId, levelName);
      if (!levelId) {
        levelId = (await invoke<string>('create_level', { level: { id: '', schoolId, name: levelName, code: null, displayOrder: nextLevelOrder(schoolId), academicYearStart: year.start, academicYearEnd: year.end, regionCode: findSchoolRegion(schoolId), timezone: browserTimezone(), defaultSyllabusId: null, defaultCalendarId: null, planningStatus: 'draft', createdAt: '', updatedAt: '', archivedAt: null } })) ?? '';
      }
      const classId = await invoke<string>('create_class', { class: { id: '', schoolId, levelId, name: className, subjectName, academicYearLabel: year.label, startDate: classStartDate, endDate: classEndDate, periodMinutes: Number(document.querySelector<HTMLInputElement>('#class-period-minutes')?.value) || 45, maxStudents: 50, schedulePatternSummary: null, yearPlanId: null, teacherNotes: null, createdAt: '', updatedAt: '', archivedAt: null } });
      if (classId) {
        await invoke<ScheduleRule[]>('set_class_schedule_rules', { classId, rules: selectedWeekdays.map((weekday) => ({ id: '', classId, weekday, startTime: null, endTime: null, periodLabel: null, isActive: true, createdAt: '', updatedAt: '' })) });
      }
      await refreshSelectedHierarchy(state, schoolId, levelId, classId ?? undefined);
      if (classId) {
        const createdSessions = await ensureAttendanceSessionsForSelectedClass(state);
        if (createdSessions > 0) await loadClassData(state);
        state.setupNotice = `${className} added. ${createdSessions} attendance date columns were prepared.`;
      } else { state.setupNotice = 'Class save finished, but no class id was returned.'; }
      showToast('Class added with attendance date columns');
    } catch (error) {
      state.setupNotice = `Could not add class: ${error instanceof Error ? error.message : String(error)}`;
      showToast('Could not add class');
    } finally { state.setupBusy = ''; render(); }
  });

  document.querySelectorAll<HTMLButtonElement>('[data-select-school]').forEach((button) => {
    button.addEventListener('click', async () => {
      state.selectedSchoolId = button.dataset.selectSchool ?? state.selectedSchoolId;
      state.selectedLevelId = button.dataset.selectLevel ?? '';
      state.selectedClassId = button.dataset.selectClass ?? '';
      await loadHierarchy(state);
      if (button.dataset.selectClass) { state.selectedClassId = button.dataset.selectClass; await loadClassData(state); }
      applySyllabusPlanningStateForCurrentContext();
      persistSyllabusPlanningState();
      render();
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-class-tab]').forEach((button) => {
    button.addEventListener('click', async () => {
      state.classTab = button.dataset.classTab as ClassTab;
      if (state.classTab === 'attendance' || state.classTab === 'calendar') {
        const createdSessions = await ensureAttendanceSessionsForSelectedClass(state);
        if (createdSessions > 0) showToast(`Created ${createdSessions} scheduled class dates`);
      }
      render();
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-class-calendar-mode]').forEach((button) => { button.addEventListener('click', () => { state.classCalendarMode = button.dataset.classCalendarMode as ClassCalendarMode; render(); }); });
  document.querySelectorAll<HTMLButtonElement>('[data-class-calendar-shift]').forEach((button) => { button.addEventListener('click', () => { shiftClassCalendar(Number(button.dataset.classCalendarShift ?? '0')); render(); }); });
  document.querySelector<HTMLButtonElement>('[data-class-calendar-today]')?.addEventListener('click', () => { state.classCalendarMonth = currentMonthKey(); render(); });
  document.querySelectorAll<HTMLElement>('.month-day.has-lesson').forEach((el) => {
    el.addEventListener('click', (e) => {
      if ((e.target as HTMLElement).closest('.lesson-btn, .lesson-actions, [data-open-lesson-plan]')) return;
      const date = el.dataset.date;
      if (!date) return;
      const existing = document.getElementById('lesson-modal');
      if (existing) existing.remove();
      const isMainCalendar = state.page === 'calendar';
      const mainCalendarSessions = isMainCalendar
        ? state.allCalendarSessions.filter((entry) => entry.session.sessionDate === date)
        : [];
      const classLessons = classCalendarLessons()
        .filter((lesson) => lesson.teachingDate === date)
        .map((lesson) => ({ lesson, className: selectedClass(state)?.name ?? '', levelName: '', schoolName: '', subjectName: null }));
      if (isMainCalendar && mainCalendarSessions.length === 0) return;
      if (!isMainCalendar && classLessons.length === 0) return;
      let html: string;
      if (isMainCalendar) {
        html = dayClassScheduleModal(date, mainCalendarSessions);
      } else if (classLessons.length === 1) {
        html = lessonDetailModal(classLessons[0].lesson, classLessons[0].className);
      } else {
        html = dayLessonsListModal(date, classLessons);
      }
      const wrapper = document.createElement('div');
      wrapper.innerHTML = html;
      document.body.appendChild(wrapper.firstElementChild!);
      const modal = document.getElementById('lesson-modal')!;
      if (!isMainCalendar) bindLessonStatusButtons(modal);
      modal.addEventListener('click', (modalEvent) => {
        if ((modalEvent.target as HTMLElement).closest('.modal-close') || modalEvent.target === modal) {
          modal.remove();
        }
      });
      modal.querySelectorAll<HTMLElement>('[data-view-lesson]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const lid = btn.dataset.viewLesson;
          const cl = classLessons.find((lesson) => lesson.lesson.id === lid);
          if (!cl) return;
          modal.innerHTML = lessonDetailModal(cl.lesson, cl.className);
          bindLessonStatusButtons(modal);
          modal.querySelector<HTMLElement>('.modal-close')?.addEventListener('click', () => modal.remove());
          modal.addEventListener('click', (me) => { if (me.target === modal) modal.remove(); });
        });
      });
    });
  });
  document.querySelectorAll<HTMLButtonElement>('[data-open-lesson-plan]').forEach((button) => {
    button.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();
      const lessonId = button.dataset.openLessonPlan ?? '';
      const lesson = classCalendarLessons().find((item) => item.id === lessonId);
      if (!lesson) return;
      const existing = document.getElementById('lesson-modal');
      if (existing) existing.remove();
      const wrapper = document.createElement('div');
      wrapper.innerHTML = lessonDetailModal(lesson, selectedClass(state)?.name);
      document.body.appendChild(wrapper.firstElementChild!);
      const modal = document.getElementById('lesson-modal')!;
      bindLessonStatusButtons(modal);
      modal.addEventListener('click', (modalEvent) => {
        if ((modalEvent.target as HTMLElement).closest('.modal-close') || modalEvent.target === modal) modal.remove();
      });
    });
  });
  bindLessonStatusButtons(document);
  document.querySelector<HTMLButtonElement>('#reschedule-missed')?.addEventListener('click', async () => {
    if (!state.selectedClassId) return;
    try {
      const lessons = await invoke<YearPlanLesson[]>('reschedule_missed_lessons', { classId: state.selectedClassId });
      state.generatedLessons = lessons ?? [];
      await loadAllCalendarLessons(state);
      await loadAllCalendarSessions(state);
      render();
      showToast('Missed lessons rescheduled');
    } catch (error) { showToast(String(error)); }
  });
  document.querySelectorAll<HTMLButtonElement>('[data-calendar-mode]').forEach((button) => { button.addEventListener('click', () => { state.calendarMode = button.dataset.calendarMode as CalendarMode; render(); }); });
  document.querySelectorAll<HTMLButtonElement>('[data-calendar-shift]').forEach((button) => { button.addEventListener('click', () => { shiftCalendar(Number(button.dataset.calendarShift ?? '0')); render(); }); });
  document.querySelector<HTMLButtonElement>('[data-calendar-today]')?.addEventListener('click', () => { state.classCalendarMonth = currentMonthKey(); render(); });
  document.querySelectorAll<HTMLButtonElement>('[data-lesson-export-scope]').forEach((button) => { button.addEventListener('click', () => { state.lessonExportScope = button.dataset.lessonExportScope as LessonExportScope; render(); }); });
  document.querySelectorAll<HTMLButtonElement>('[data-lesson-export-format]').forEach((button) => { button.addEventListener('click', () => { state.lessonExportFormat = button.dataset.lessonExportFormat as LessonExportFormat; render(); }); });

  document.querySelector<HTMLInputElement>('#lesson-template')?.addEventListener('change', async (event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    state.lessonTemplate = await file.text();
    state.lessonTemplateName = file.name;
    render();
  });

  document.querySelector<HTMLTextAreaElement>('#lesson-context-notes')?.addEventListener('input', (event) => {
    state.lessonContextNotes = (event.currentTarget as HTMLTextAreaElement).value;
    try { localStorage.setItem('edutrack.lessonContextNotes', state.lessonContextNotes); } catch { /* noop */ }
  });

  document.querySelector<HTMLButtonElement>('#generate-detailed-plans')?.addEventListener('click', async () => {
    await generateDetailedPlansFromSyllabus();
  });

  document.querySelector<HTMLInputElement>('#lesson-select-all')?.addEventListener('change', (event) => {
    const checked = (event.currentTarget as HTMLInputElement).checked;
    const groups = groupLessonsForExport(sortedGeneratedLessons(), state.lessonExportScope);
    lessonExportSelection.clear();
    if (checked) {
      groups.forEach((group, index) => {
        lessonExportSelection.add(exportGroupSelectionId(state.lessonExportScope, group, index));
      });
    }
    render();
  });

  document.querySelectorAll<HTMLInputElement>('[data-export-select]').forEach((input) => {
    input.addEventListener('change', () => {
      const id = input.dataset.exportSelect ?? '';
      if (!id) return;
      if (input.checked) lessonExportSelection.add(id);
      else lessonExportSelection.delete(id);
      render();
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-preview-export]').forEach((button) => {
    button.addEventListener('click', () => {
      const id = button.dataset.previewExport ?? '';
      if (!id) return;
      const groups = groupLessonsForExport(sortedGeneratedLessons(), state.lessonExportScope);
      const index = groups.findIndex((group, idx) => exportGroupSelectionId(state.lessonExportScope, group, idx) === id);
      if (index < 0) return;
      const existing = document.getElementById('lesson-modal');
      if (existing) existing.remove();
      const wrapper = document.createElement('div');
      wrapper.innerHTML = exportGroupPreviewModal(groups[index], state.lessonExportScope);
      document.body.appendChild(wrapper.firstElementChild!);
      const modal = document.getElementById('lesson-modal')!;
      bindLessonStatusButtons(modal);
      modal.addEventListener('click', (event) => {
        if ((event.target as HTMLElement).closest('.modal-close') || event.target === modal) modal.remove();
      });
    });
  });

  document.querySelector<HTMLButtonElement>('#export-selected-groups')?.addEventListener('click', () => {
    const groups = groupLessonsForExport(sortedGeneratedLessons(), state.lessonExportScope);
    const selectedGroups = selectedLessonExportGroups(groups, state.lessonExportScope);
    if (selectedGroups.length === 0) return;
    const context = {
      school: selectedSchool(state)?.name ?? 'School',
      className: selectedClass(state)?.name ?? 'Class',
      level: selectedLevel(state)?.name ?? 'Level',
      scope: `${exportScopeTitle(state.lessonExportScope)} (${selectedGroups.length} selected)`,
    };
    const { content, ext, mime } = buildLessonExport(context, selectedGroups, state.lessonExportFormat);
    const className = selectedClass(state)?.name?.replace(/[^a-z0-9]+/gi, "-").toLowerCase() || "class";
    downloadExport(`${className}-lesson-plans-${state.lessonExportScope}-selected`, content, mime, ext, state.lessonExportFormat);
    showToast(`Exported ${selectedGroups.length} group${selectedGroups.length === 1 ? '' : 's'} as ${state.lessonExportFormat.toUpperCase()}`);
  });

  document.querySelectorAll<HTMLButtonElement>('[data-attendance-cell]').forEach((button) => {
    button.addEventListener('click', async () => {
      const sessionId = button.dataset.sessionId ?? '';
      const studentId = button.dataset.studentId ?? '';
      if (!sessionId || !studentId) return;
      const current = attendanceFor(studentId, sessionId);
      const status = nextAttendanceStatus(current?.status ?? '');
      try {
        await invoke<string>('create_attendance_record', { sessionId, studentId, status, note: null });
        await loadAttendanceData(state);
        render();
      } catch (error) { showToast(`Could not update attendance: ${error instanceof Error ? error.message : String(error)}`); }
    });
  });

  document.querySelector<HTMLButtonElement>('#toggle-attendance-history')?.addEventListener('click', () => { state.showAttendanceHistory = !state.showAttendanceHistory; render(); });

  document.querySelector<HTMLButtonElement>('#sync-holidays')?.addEventListener('click', async () => {
    const level = selectedLevel(state);
    if (!level) return;
    try {
      const { closures } = await syncPublicHolidaysForLevel(state, level);
      render();
      showToast(`Synced ${closures.filter((c) => c.closureType === 'holiday').length} public holidays`);
    } catch (error) { showToast(`Holiday sync failed: ${error instanceof Error ? error.message : String(error)}`); }
  });

  const providerSelect = document.querySelector<HTMLSelectElement>('#provider-select');
  providerSelect?.addEventListener('change', async () => {
    await loadModels(state, providerSelect.value);
    if (state.llmConfig) { state.llmConfig.provider = providerSelect.value as 'ollama' | 'opencode'; state.llmConfig.model = state.llmModels[0]?.id ?? ''; }
    render();
  });

  document.querySelector<HTMLFormElement>('#provider-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const provider = document.querySelector<HTMLSelectElement>('#provider-select')?.value as 'ollama' | 'opencode';
    const model = document.querySelector<HTMLSelectElement>('#model-select')?.value ?? '';
    const saved = await invoke<LlmConfig>('set_llm_provider', { provider, model });
    state.llmConfig = saved ?? state.llmConfig;
    if (state.llmConfig) {
      await loadModels(state, state.llmConfig.provider);
      await refreshGradingDiagnostics();
      showToast('Settings saved successfully');
    }
    render();
  });

  document.querySelector<HTMLButtonElement>('#refresh-diagnostics')?.addEventListener('click', async () => {
    await refreshGradingDiagnostics();
    render();
    showToast('Diagnostics refreshed');
  });

  document.querySelector<HTMLButtonElement>('#install-grading-tools')?.addEventListener('click', async () => {
    await installMissingGradingTools();
  });
  document.querySelectorAll<HTMLButtonElement>('[data-install-tool]').forEach((button) => {
    button.addEventListener('click', async () => {
      const toolKey = button.dataset.installTool ?? '';
      const label = button.dataset.installToolLabel ?? toolKey;
      await installSingleGradingTool(toolKey, label);
    });
  });

  document.querySelector<HTMLInputElement>('#teacher-image')?.addEventListener('change', async (event) => {
    const input = event.currentTarget as HTMLInputElement;
    const preview = document.querySelector<HTMLDivElement>('#teacher-image-preview');
    const file = input.files?.[0];
    if (!preview || !file) {
      if (preview) preview.innerHTML = state.teacherProfile.image ? `<img src="${escapeHtml(state.teacherProfile.image)}" alt="Current teacher profile image" /><span>Current image</span>` : '<span>No image selected</span>';
      return;
    }
    try {
      const dataUrl = await readImageAsDataUrl(file);
      preview.innerHTML = `<img src="${escapeHtml(dataUrl)}" alt="Selected teacher profile image preview" /><span>${escapeHtml(file.name)}</span>`;
    } catch (error) { input.value = ''; preview.innerHTML = '<span>No image selected</span>'; showToast(error instanceof Error ? error.message : String(error)); }
  });

  document.querySelector<HTMLFormElement>('#teacher-profile-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const name = document.querySelector<HTMLInputElement>('#teacher-name')?.value.trim() || 'Teacher';
    const title = document.querySelector<HTMLInputElement>('#teacher-title')?.value.trim() || 'Teacher workspace';
    const imageFile = document.querySelector<HTMLInputElement>('#teacher-image')?.files?.[0] ?? null;
    let image = state.teacherProfile.image ?? null;
    if (imageFile) {
      try {
        const buffer = await imageFile.arrayBuffer();
        const fileData = Array.from(new Uint8Array(buffer));
        image = await invoke<string>('save_image_file', { fileData, fileName: imageFile.name });
      } catch (error) { showToast(error instanceof Error ? error.message : String(error)); return; }
    }
    state.teacherProfile = { name, title, image };
    saveTeacherProfile(state.teacherProfile);
    if (state.backend) invoke('save_teacher_profile', { profileJson: JSON.stringify(state.teacherProfile) });
    render();
    showToast('Teacher profile saved');
  });

  document.querySelector<HTMLButtonElement>('#run-agents')?.addEventListener('click', async () => {
    if (!state.selectedClassId) return;
    const selectedAgents = Array.from(document.querySelectorAll<HTMLInputElement>('[data-agent-toggle]')).map((input) => ({ agentType: input.dataset.agentToggle ?? '', enabled: input.checked, configJson: null }));
    await invoke('update_class_agents', { classId: state.selectedClassId, agents: selectedAgents });
    state.insights = (await invoke<AgentInsight[]>('run_class_agents', { classId: state.selectedClassId })) ?? [];
    showToast('Agents finished analyzing classroom data');
    render();
  });

  document.querySelector<HTMLButtonElement>('#upload-syllabus')?.addEventListener('click', async () => {
    const file = document.querySelector<HTMLInputElement>('#syllabus-file')?.files?.[0];
    const levelId = document.querySelector<HTMLSelectElement>('#syllabus-level')?.value || state.selectedLevelId;
    const classId = document.querySelector<HTMLSelectElement>('#syllabus-class')?.value || state.selectedClassId;
    const level = state.levels.find((item) => item.id === levelId);
    if (!state.backend) { showToast('Open the desktop app to process syllabi'); return; }
    if (!file || !level || !classId) return;
    const scheduleRules = (await invoke<ScheduleRule[]>('get_class_schedule_rules', { classId })) ?? [];
    if (scheduleRules.length === 0) { showToast('Set class meeting days before building a lesson plan'); return; }

    state.syllabusBusy = true;
    state.planMessage = '';
    state.extractedUnits = [];
    state.syllabusExtractionDiagnostics = null;
    state.currentSyllabusId = '';
    persistSyllabusPlanningState();
    render();

    let toastMessage = '';
    try {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      const syllabusId = await invoke<string>('upload_syllabus', { levelId, fileName: file.name, fileData: bytes });
      if (syllabusId) {
        const units = (await invoke<CurriculumUnit[]>('extract_curriculum_units', { syllabusId })) ?? [];
        state.currentSyllabusId = syllabusId;
        state.extractedUnits = units;
        await refreshSyllabusExtractionDiagnostics(syllabusId);
        state.generatedLessons = [];
        const monthCount = units.length > 0 ? (units[0].scheduleSlotCount ?? units.length) : 0;
        state.planMessage = `Extracted ${units.length} sections \u00b7 ${monthCount} months. Review them, then generate the lesson schedule.`;
        state.selectedLevelId = levelId;
        state.selectedClassId = classId;
        toastMessage = 'Syllabus uploaded and extracted successfully.';
        try { state.insights = (await invoke<AgentInsight[]>('run_class_agents', { classId })) ?? []; } catch {}
      } else { state.message = 'Upload completed without an id.'; }
    } catch (error) { state.planMessage = `Syllabus extraction failed: ${error instanceof Error ? error.message : String(error)}`; toastMessage = 'Could not extract syllabus'; }
    state.syllabusBusy = false;
    persistSyllabusPlanningState();
    render();
    if (toastMessage) showToast(toastMessage);
  });

  document.querySelector<HTMLButtonElement>('#generate-reviewed-plan')?.addEventListener('click', async () => {
    const levelId = document.querySelector<HTMLSelectElement>('#syllabus-level')?.value || state.selectedLevelId;
    const classId = document.querySelector<HTMLSelectElement>('#syllabus-class')?.value || state.selectedClassId;
    const level = state.levels.find((item) => item.id === levelId);
    if (levelId) state.selectedLevelId = levelId;
    if (classId) state.selectedClassId = classId;
    if (!state.currentSyllabusId) {
      showToast('Upload a syllabus before generating a lesson schedule.');
      return;
    }
    if (!level) {
      showToast('Select a valid level before generating a lesson schedule.');
      return;
    }
    if (!classId) {
      showToast('Select a class before generating a lesson schedule.');
      return;
    }
    const reviewedUnits = collectReviewedUnits();
    if (reviewedUnits.length === 0) {
      showToast('No syllabus sections selected. Please select at least one syllabus section before generating a schedule.');
      return;
    }
    state.planMessage = 'Saving reviewed sections and building lesson schedule...';
    render();
    try {
      await invoke('update_syllabus_review', { syllabusId: state.currentSyllabusId, units: reviewedUnits, markReviewed: true });
      const calendarId = await ensureCalendarForLevel(state, level);
      let holidayNotice = 'Public holiday sync unavailable; used existing calendar closure days.';
      try {
        const { closures } = await syncPublicHolidaysForLevel(state, level);
        holidayNotice = `Synced ${closures.filter((c) => c.closureType === 'holiday').length} public holidays.`;
      } catch {
        showToast('Public holiday sync failed. Continuing with existing calendar closure days.');
      }
      const bufferInput = document.querySelector<HTMLInputElement>('#schedule-buffer-percent');
      const pacingInput = document.querySelector<HTMLSelectElement>('#schedule-pacing-mode');
      const bufferPercent = bufferInput ? Math.max(0, Math.min(25, Number(bufferInput.value) || 8)) / 100 : 0.08;
      const pacingMode = pacingInput?.value || 'balanced';
      const plan = await invoke<YearPlanBundle>('generate_year_plan', { options: { levelId, classId, syllabusId: state.currentSyllabusId, calendarId, bufferDaysPercent: bufferPercent, pacingMode } });
      state.generatedLessons = plan?.lessons ?? [];
      state.planMessage = `${plan?.yearPlan.generationSummary ?? `Generated ${state.generatedLessons.length} scheduled lessons.`} ${holidayNotice}`;
      if (plan?.warnings.length) state.planMessage = `${state.planMessage} ${plan.warnings.join(' ')}`;
      state.classes = (await invoke<ClassView[]>('get_classes_by_level', { levelId })) ?? [];
      await loadClassData(state);
      await loadSchoolDirectory(state);
      await loadAllCalendarLessons(state);
      await loadAllCalendarSessions(state);
      persistSyllabusPlanningState();
      render();
      showToast('Lesson schedule generated and added to the classroom calendar.');
    } catch (error) { state.planMessage = `Schedule generation failed: ${error instanceof Error ? error.message : String(error)}`; persistSyllabusPlanningState(); render(); showToast('Could not generate lesson schedule'); }
  });

  document.querySelector<HTMLButtonElement>('#syllabus-refresh-diagnostics')?.addEventListener('click', async () => {
    if (!state.currentSyllabusId) return;
    await refreshSyllabusExtractionDiagnostics(state.currentSyllabusId);
    render();
    if (state.syllabusExtractionDiagnostics) {
      showToast('Syllabus extraction diagnostics refreshed.');
    }
  });

  document.querySelector<HTMLButtonElement>('#syllabus-export-text')?.addEventListener('click', async () => {
    if (!state.currentSyllabusId) return;
    try {
      const text = await invoke<string>('get_syllabus_raw_text', { syllabusId: state.currentSyllabusId });
      if (text) {
        const blob = new Blob([text], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'syllabus-raw-text.txt';
        a.click();
        URL.revokeObjectURL(url);
        showToast('Raw syllabus text exported.');
      }
    } catch (e) {
      showToast('Failed to export raw text.');
    }
  });

  document.querySelector<HTMLButtonElement>('#syllabus-generate-detailed-plans')?.addEventListener('click', async () => {
    await generateDetailedPlansFromSyllabus();
  });

  document.querySelector<HTMLButtonElement>('#syllabus-attach-detailed-plans')?.addEventListener('click', async () => {
    await attachDetailedPlansToCalendar();
  });
  document.querySelector<HTMLButtonElement>('#cancel-detailed-plan-generation')?.addEventListener('click', () => {
    if (!state.lessonPlansBusy || detailedPlanCancelRequested) return;
    detailedPlanCancelRequested = true;
    state.detailedPlanProgressMessage = 'Cancellation requested. Stopping after the current lesson';
    updateDetailedPlanProgressOverlay();
    const button = document.querySelector<HTMLButtonElement>('#cancel-detailed-plan-generation');
    if (button) {
      button.disabled = true;
      button.textContent = 'Cancelling...';
    }
    showToast('Cancellation requested. EduTrack will stop after the current lesson finishes.');
  });

  document.querySelector<HTMLSelectElement>('#syllabus-school')?.addEventListener('change', async (event) => {
    state.selectedSchoolId = (event.currentTarget as HTMLSelectElement).value;
    state.selectedLevelId = '';
    state.selectedClassId = '';
    await loadHierarchy(state);
    applySyllabusPlanningStateForCurrentContext();
    persistSyllabusPlanningState();
    render();
  });
  document.querySelector<HTMLSelectElement>('#syllabus-level')?.addEventListener('change', async (event) => {
    state.selectedLevelId = (event.currentTarget as HTMLSelectElement).value;
    state.selectedClassId = '';
    await loadHierarchy(state);
    applySyllabusPlanningStateForCurrentContext();
    persistSyllabusPlanningState();
    render();
  });
  document.querySelector<HTMLSelectElement>('#syllabus-class')?.addEventListener('change', async (event) => {
    state.selectedClassId = (event.currentTarget as HTMLSelectElement).value;
    await loadClassData(state);
    applySyllabusPlanningStateForCurrentContext();
    persistSyllabusPlanningState();
    render();
  });

  document.querySelector<HTMLFormElement>('#test-create-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (!state.selectedClassId) { showToast('Select a class first'); return; }
    const title = (document.querySelector<HTMLInputElement>('#test-title')?.value ?? '').trim();
    const instructions = (document.querySelector<HTMLTextAreaElement>('#test-instructions')?.value ?? '').trim();
    const rubric = (document.querySelector<HTMLTextAreaElement>('#test-rubric')?.value ?? '').trim();
    const maxScore = Number(document.querySelector<HTMLInputElement>('#test-max-score')?.value ?? 100);
    if (!title || !instructions) { showToast('Enter a test title and instructions'); return; }
    try {
      const id = await invoke<string>('create_test', { classId: state.selectedClassId, title, instructions, scoringRubric: rubric, maxScore });
      if (id) { state.currentTestId = id; state.tests = (await invoke<TestTemplate[]>('get_class_tests', { classId: state.selectedClassId })) ?? []; state.testSubmissions = (await invoke<StudentSubmission[]>('get_test_submissions', { assessmentId: id })) ?? []; showToast('Test created'); render(); }
    } catch (error) { showToast(String(error)); }
  });

  document.querySelector<HTMLButtonElement>('#test-generate-syllabus')?.addEventListener('click', async () => {
    if (!state.selectedClassId) { showToast('Select a class first'); return; }
    const sections = collectSyllabusSectionsForTestDraft();
    if (sections.length === 0) {
      showToast('No syllabus sections loaded. Go to Syllabus page, upload/review sections, then try again.');
      return;
    }
    const selectedSections = await openTestSyllabusSectionPickerModal(sections);
    if (!selectedSections) return;
    if (selectedSections.length === 0) {
      showToast('Select at least one syllabus section.');
      return;
    }
    try {
      const draft = await invoke<TestDraft>('create_test_based_on_syllabus', {
        classId: state.selectedClassId,
        selectedSections,
      });
      if (!draft) return;
      const titleInput = document.querySelector<HTMLInputElement>('#test-title');
      const instructionsInput = document.querySelector<HTMLTextAreaElement>('#test-instructions');
      const rubricInput = document.querySelector<HTMLTextAreaElement>('#test-rubric');
      const maxScoreInput = document.querySelector<HTMLInputElement>('#test-max-score');
      if (titleInput) titleInput.value = draft.title;
      if (instructionsInput) instructionsInput.value = draft.instructions;
      if (rubricInput) rubricInput.value = draft.scoringRubric;
      if (maxScoreInput) maxScoreInput.value = String(Math.round(draft.maxScore));
      showToast('Syllabus-based test draft ready. Review and click Create test.');
    } catch (error) { showToast(String(error)); }
  });

  document.querySelectorAll<HTMLElement>('[data-select-test]').forEach((el) => {
    el.addEventListener('click', async () => {
      const id = el.dataset.selectTest ?? '';
      if (!id) return;
      state.currentTestId = id;
      state.testSubmissions = (await invoke<StudentSubmission[]>('get_test_submissions', { assessmentId: id })) ?? [];
      render();
    });
  });

  document.querySelector<HTMLButtonElement>('#auto-score-test')?.addEventListener('click', async () => {
    if (!state.currentTestId) return;
    try {
      const results = await invoke<StudentSubmission[]>('auto_score_submissions', { assessmentId: state.currentTestId });
      if (results) state.testSubmissions = results;
      showToast('Auto-scoring complete â€” review and adjust scores as needed');
      render();
    } catch (error) { showToast(String(error)); }
  });

  document.querySelector<HTMLButtonElement>('#export-test-results')?.addEventListener('click', () => {
    if (!state.currentTestId || state.testSubmissions.length === 0) return;
    const test = state.tests.find((t) => t.id === state.currentTestId);
    let csv = 'Student,Score,Label,Feedback,File\n';
    for (const sub of state.testSubmissions) {
      csv += `"${sub.studentName}",${sub.scoreValue ?? ''},"${sub.scoreLabel ?? ''}","${(sub.teacherComment ?? '').replace(/"/g, '""')}","${sub.fileName ?? ''}"\n`;
    }
    downloadTextFile(`${test?.title ?? 'test'}-results.csv`, csv, 'text/csv');
    showToast('Results exported');
  });

  document.querySelectorAll<HTMLButtonElement>('[data-save-score]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const studentId = btn.dataset.saveScore ?? '';
      if (!studentId || !state.currentTestId) return;
      const scoreInput = document.querySelector<HTMLInputElement>(`[data-score-student="${studentId}"]`);
      const commentInput = document.querySelector<HTMLTextAreaElement>(`[data-comment-student="${studentId}"]`);
      const score = scoreInput ? parseFloat(scoreInput.value) : undefined;
      const comment = commentInput?.value?.trim() || null;
      if (score === undefined || isNaN(score)) { showToast('Enter a valid score'); return; }
      try {
        await invoke('score_submission', { assessmentId: state.currentTestId, studentId, score, label: null, comment });
        showToast('Score saved');
        state.testSubmissions = (await invoke<StudentSubmission[]>('get_test_submissions', { assessmentId: state.currentTestId })) ?? [];
        render();
      } catch (error) { showToast(String(error)); }
    });
  });

  document.querySelectorAll<HTMLInputElement>('[data-upload-student]').forEach((input) => {
    input.addEventListener('change', async (event) => {
      const fileInput = event.currentTarget as HTMLInputElement;
      const studentId = fileInput.dataset.uploadStudent ?? '';
      const file = fileInput.files?.[0];
      if (!file || !studentId || !state.currentTestId) return;
      try {
        const buffer = await file.arrayBuffer();
        const fileData = Array.from(new Uint8Array(buffer));
        await invoke('upload_submission_file', { assessmentId: state.currentTestId, studentId, fileName: file.name, fileData, mimeType: file.type });
        showToast(`File uploaded for ${file.name}`);
        state.testSubmissions = (await invoke<StudentSubmission[]>('get_test_submissions', { assessmentId: state.currentTestId })) ?? [];
        render();
      } catch (error) { showToast(String(error)); }
    });
  });

  document.querySelectorAll<HTMLInputElement>('[data-upload-matrix]').forEach((input) => {
    input.addEventListener('change', async (event) => {
      const fileInput = event.currentTarget as HTMLInputElement;
      const assessmentId = fileInput.dataset.matrixAssessment ?? '';
      const studentId = fileInput.dataset.matrixStudent ?? '';
      const file = fileInput.files?.[0];
      if (!assessmentId || !studentId || !file) return;
      try {
        const buffer = await file.arrayBuffer();
        const fileData = Array.from(new Uint8Array(buffer));
        await invoke('upload_submission_file', { assessmentId, studentId, fileName: file.name, fileData, mimeType: file.type });
        const refreshed = (await invoke<StudentSubmission[]>('get_test_submissions', { assessmentId })) ?? [];
        state.classroomTestSubmissions[assessmentId] = refreshed;
        if (state.currentTestId === assessmentId) state.testSubmissions = refreshed;
        showToast(`Uploaded ${file.name}`);
        render();
      } catch (error) { showToast(String(error)); }
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-view-file]').forEach((btn) => {
    btn.addEventListener('click', () => { showToast('File preview coming soon â€” files are saved to the local data directory'); });
  });

  document.querySelector<HTMLInputElement>('#report-instructions')?.addEventListener('input', (e) => { state.reportInstructions = (e.currentTarget as HTMLInputElement).value; });
  document.querySelector<HTMLInputElement>('#report-word-count')?.addEventListener('change', (e) => { state.reportWordCount = Number((e.currentTarget as HTMLInputElement).value) || 300; });

  document.querySelector<HTMLButtonElement>('#generate-all-reports')?.addEventListener('click', async () => {
    if (!state.selectedClassId || state.students.length === 0) return;
    const btn = document.querySelector<HTMLButtonElement>('#generate-all-reports');
    if (btn) { btn.disabled = true; btn.textContent = 'Generating...'; }
    let success = 0;
    let failed = 0;
    for (const student of state.students) {
      try { await invoke<string>('generate_student_report_v2', { studentId: student.id, classId: state.selectedClassId, teacherInstructions: state.reportInstructions.trim() || null, wordCountTarget: state.reportWordCount }); success++; }
      catch { failed++; }
    }
    state.reportStatuses = (await invoke<StudentReportStatus[]>('get_class_report_status', { classId: state.selectedClassId })) ?? [];
    if (btn) { btn.disabled = false; btn.innerHTML = "${icon('book')} Generate reports for all students"; }
    showToast(`Reports generated: ${success} success${success === 1 ? '' : 'es'}, ${failed} failed`);
    render();
  });

  document.querySelectorAll<HTMLButtonElement>('[data-view-report]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const reportId = btn.dataset.viewReport ?? '';
      if (!reportId) return;
      try {
        const report = await invoke<StudentReportView>('get_student_report', { reportId });
        if (!report) { showToast('Report not found'); return; }
        const content = `<div class="panel"><div class="panel-head"><h2>Student Report</h2><p class="eyebrow">Generated</p></div><div style="white-space:pre-wrap;font-size:14px;line-height:1.6">${escapeHtml(report.reportText)}</div>${report.teacherAdviceText ? `<hr style="margin:var(--space-4) 0;border-color:var(--line)"><div><p class="label">Teacher advice</p><p style="white-space:pre-wrap;font-size:13px;color:var(--muted)">${escapeHtml(report.teacherAdviceText)}</p></div>` : ''}</div>`;
        const reportHtml = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>Student Report</title><link rel="stylesheet" href="${window.location.href.replace(/\/[^/]*$/, '/')}src/style.css"></head><body style="max-width:800px;margin:40px auto;padding:0 20px">${content}</body></html>`;
        const reportBlob = new Blob([reportHtml], { type: 'text/html;charset=utf-8' });
        const reportUrl = URL.createObjectURL(reportBlob);
        window.open(reportUrl, '_blank');
        setTimeout(() => URL.revokeObjectURL(reportUrl), 30000);
      } catch (error) { showToast(String(error)); }
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-export-report]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const reportId = btn.dataset.exportReport ?? '';
      if (!reportId) { showToast('Report not found'); return; }
      try {
        const report = await invoke<StudentReportView>('get_student_report', { reportId });
        if (!report) { showToast('Report not found'); return; }
        const txt = `${report.reportText}\n\n${report.teacherAdviceText ? `Teacher advice:\n${report.teacherAdviceText}` : ''}`;
        downloadTextFile(`report-${reportId.slice(0, 8)}.txt`, txt, 'text/plain');
        showToast('Report exported');
      } catch (error) { showToast(String(error)); }
    });
  });

  document.querySelector<HTMLSelectElement>('#clear-school-select')?.addEventListener('change', () => {
    const btn = document.querySelector<HTMLButtonElement>('#clear-school-btn');
    if (btn) btn.disabled = !document.querySelector<HTMLSelectElement>('#clear-school-select')?.value;
  });

  document.querySelector<HTMLButtonElement>('#clear-school-btn')?.addEventListener('click', async () => {
    const schoolId = document.querySelector<HTMLSelectElement>('#clear-school-select')?.value;
    if (!schoolId) return;
    const school = state.schools.find((s) => s.id === schoolId);
    if (!school) return;
    const dangerConfirm = await openConfirmModal({
      id: 'confirm-delete-school-data',
      title: `Delete all data for ${school.name}?`,
      message: 'This permanently removes levels, classes, students, attendance, assessments, reports, syllabi, calendars, and related records. This cannot be undone.',
      confirmLabel: 'Continue',
      cancelLabel: 'Cancel',
      confirmClass: 'danger',
    });
    if (!dangerConfirm) return;
    const typed = await openTextEntryModal({
      id: 'confirm-delete-school-data-typed',
      title: 'Type School Name To Confirm',
      message: `Enter "${school.name}" to permanently delete all school data.`,
      placeholder: school.name,
      confirmLabel: 'Delete permanently',
      cancelLabel: 'Cancel',
    });
    if (typed !== school.name) { showToast('Cancelled: school name did not match'); return; }
    try {
      const deleted = await invoke<DeleteSchoolDataResult>('delete_school_data', { schoolId });
      state.schools = (await invoke<School[]>('get_schools')) ?? [];
      await loadSchoolImages(state);
      state.selectedSchoolId = state.schools[0]?.id ?? '';
      state.selectedLevelId = '';
      state.selectedClassId = '';
      await loadHierarchy(state);
      await loadSchoolDirectory(state);
      await loadAllCalendarLessons(state);
      await loadAllCalendarSessions(state);
      render();
      const plans = deleted?.yearPlansDeleted ?? 0;
      const schedules = deleted?.yearPlanLessonsDeleted ?? 0;
      showToast(`Deleted ${school.name}: ${plans} lesson plan set${plans === 1 ? '' : 's'}, ${schedules} scheduled lesson${schedules === 1 ? '' : 's'}.`);
    } catch (error) { showToast(String(error)); }
  });
}

function bindCountryPicker() {
  const picker = document.querySelector<HTMLElement>('[data-country-picker]');
  if (!picker) return;

  const trigger = picker.querySelector<HTMLButtonElement>('[data-country-trigger]');
  const popover = picker.querySelector<HTMLDivElement>('[data-country-popover]');
  const filter = picker.querySelector<HTMLInputElement>('[data-country-filter]');
  const input = picker.querySelector<HTMLInputElement>('#school-region');
  const selectedLabel = picker.querySelector<HTMLElement>('[data-country-selected-label]');
  const selectedFlag = picker.querySelector<HTMLElement>('[data-country-selected-flag]');
  const options = Array.from(picker.querySelectorAll<HTMLButtonElement>('[data-country-option]'));

  if (!trigger || !popover || !filter || !input || !selectedLabel || !selectedFlag) return;

  ensureCountryPickerOutsideBinding();

  const setExpanded = (expanded: boolean) => {
    trigger.setAttribute('aria-expanded', expanded ? 'true' : 'false');
    popover.hidden = !expanded;
    picker.dataset.countryOpen = expanded ? '1' : '0';
    if (expanded) {
      activeCountryPickerClose = () => setExpanded(false);
    } else if (activeCountryPickerClose) {
      activeCountryPickerClose = null;
    }
    if (expanded) filter.focus();
  };

  const applySelection = (code: string, name: string) => {
    input.value = code;
    selectedLabel.textContent = name;
    selectedFlag.className = `fi fi-${code.toLowerCase()} country-flag`;
    options.forEach((option) => {
      const isActive = option.dataset.countryOption === code;
      option.classList.toggle('active', isActive);
      option.setAttribute('aria-selected', isActive ? 'true' : 'false');
    });
  };

  trigger.addEventListener('click', () => {
    const isExpanded = trigger.getAttribute('aria-expanded') === 'true';
    setExpanded(!isExpanded);
  });

  options.forEach((option) => {
    option.addEventListener('click', () => {
      const code = option.dataset.countryOption ?? '';
      const name = option.querySelector('span:last-child')?.textContent ?? code;
      if (!code) return;
      applySelection(code, name);
      filter.value = '';
      options.forEach((entry) => { entry.hidden = false; });
      setExpanded(false);
      trigger.focus();
    });
  });

  filter.addEventListener('input', () => {
    const query = filter.value.trim().toLowerCase();
    options.forEach((option) => {
      const name = option.dataset.countryName ?? '';
      option.hidden = query.length > 0 && !name.includes(query);
    });
  });

  picker.addEventListener('focusout', (event) => {
    const nextTarget = event.relatedTarget as Node | null;
    if (!nextTarget || !picker.contains(nextTarget)) setExpanded(false);
  });

  picker.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      setExpanded(false);
      trigger.focus();
    }
  });
}

window.addEventListener('beforeunload', (event) => {
  if (!state.lessonPlansBusy) return;
  event.preventDefault();
  event.returnValue = '';
});

void loadInitialData();



