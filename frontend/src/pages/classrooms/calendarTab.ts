import type {
  CalendarClassSessionView,
  CalendarClosureDay,
  ClassCalendarMode,
  ScheduleRule,
  SessionView,
  YearPlanLesson,
} from '../../types';
import { formatYmdUtc, isTodayIso, monthGridDates, parseYmdUtc } from '../../utils';

type CalendarLessonListEntry = {
  lesson: YearPlanLesson;
  className: string;
  schoolName: string;
  subjectName: string | null;
};

type LessonPlanLike = {
  objective: string;
  transferGoal: string;
  enduringUnderstanding: string;
  vocabulary: string[];
  essentialQuestions: string[];
  knowledge: string[];
  skills: string[];
  successCriteria: string[];
  assessment: string;
  performanceTask?: string;
  lessonFlow: Array<{ phase: string; time: string; description: string }>;
  materials: string[];
  differentiation: string;
  homework?: string;
  crossCurricular?: string;
  backwardDesign?: unknown;
};

type RenderClassCalendarTabParams = {
  className: string;
  schoolName: string;
  classCalendarMode: ClassCalendarMode;
  classCalendarMonth: string;
  scheduleRules: ScheduleRule[];
  generatedLessons: YearPlanLesson[];
  calendarClosures: CalendarClosureDay[];
  classSessions: SessionView[];
  classLessons: YearPlanLesson[];
  academicLabel: string;
  weekLabels: string[];
  escapeHtml: (value: string) => string;
  capitalize: (value: string) => string;
  icon: (name: string) => string;
};

type RenderLessonDetailModalParams = {
  lesson: YearPlanLesson;
  className?: string;
  periodMinutes: number;
  teacherName: string;
  parseLessonPlan: (value: string | null | undefined) => LessonPlanLike | null;
  escapeHtml: (value: string) => string;
  capitalize: (value: string) => string;
  icon: (name: string) => string;
};

type BindClassCalendarTabEventsParams = {
  onSetMode: (mode: ClassCalendarMode) => void;
  onShift: (value: number) => void;
  onToday: () => void;
  isMainCalendarPage: () => boolean;
  getMainCalendarSessionsByDate: (date: string) => CalendarClassSessionView[];
  getClassLessonsByDate: (date: string) => CalendarLessonListEntry[];
  getClassLessonById: (lessonId: string) => YearPlanLesson | null;
  selectedClassName: () => string;
  renderLessonDetailModal: (lesson: YearPlanLesson, className?: string) => string;
  renderDayClassScheduleModal: (date: string, sessions: CalendarClassSessionView[]) => string;
  renderDayLessonsListModal: (date: string, lessons: CalendarLessonListEntry[]) => string;
  bindLessonStatusButtons: (scope: ParentNode) => void;
  onRescheduleMissed: () => Promise<void>;
};

export function classCalendarCursor(classCalendarMonth: string): Date {
  return parseYmdUtc(`${classCalendarMonth}-01`) ?? new Date(Date.UTC(new Date().getFullYear(), new Date().getMonth(), 1));
}

export function shiftClassCalendarMonth(
  classCalendarMonth: string,
  classCalendarMode: ClassCalendarMode,
  value: number,
): string {
  const cursor = classCalendarCursor(classCalendarMonth);
  cursor.setUTCMonth(cursor.getUTCMonth() + (classCalendarMode === 'year' ? value * 12 : value));
  return `${cursor.getUTCFullYear()}-${`${cursor.getUTCMonth() + 1}`.padStart(2, '0')}`;
}

export function classCalendarLessonsForSelectedClass(params: {
  selectedClassId: string;
  generatedLessons: YearPlanLesson[];
  allCalendarLessons: Array<{ classId: string; lesson: YearPlanLesson }>;
}): YearPlanLesson[] {
  const { selectedClassId, generatedLessons, allCalendarLessons } = params;
  if (!selectedClassId) return generatedLessons;
  const lessons = allCalendarLessons
    .filter((entry) => entry.classId === selectedClassId)
    .map((entry) => entry.lesson);
  return lessons.length > 0 ? lessons : generatedLessons;
}

export function renderClassCalendarTab(params: RenderClassCalendarTabParams): string {
  const {
    className,
    schoolName,
    classCalendarMode,
    classCalendarMonth,
    scheduleRules,
    generatedLessons,
    calendarClosures,
    classSessions,
    classLessons,
    academicLabel,
    weekLabels,
    escapeHtml,
    capitalize,
    icon,
  } = params;

  const cursor = classCalendarCursor(classCalendarMonth);
  const year = cursor.getUTCFullYear();
  const month = cursor.getUTCMonth();
  const monthTitle = cursor.toLocaleDateString(undefined, { month: 'long', year: 'numeric', timeZone: 'UTC' });
  const scheduledClassDays = classSessions.length;
  const todayIso = formatYmdUtc(new Date());
  const completedLessons = classLessons.filter((lesson) => lesson.status === 'completed').length;
  const missedLessons = classLessons.filter((lesson) => lesson.status === 'missed').length;
  const upcomingLessons = classLessons.filter((lesson) => lesson.teachingDate >= todayIso && lesson.status !== 'completed').length;
  const scheduleSummary = scheduleRules.length === 0
    ? 'No meeting days set'
    : scheduleRules.map((rule) => [capitalize(rule.weekday), rule.periodLabel, rule.startTime].filter(Boolean).join(' ')).join(', ');

  const monthCalendar = renderMonthCalendar({
    year,
    month,
    compact: false,
    weekLabels,
    classSessions,
    classLessons,
    calendarClosures,
    escapeHtml,
  });

  const yearCalendar = renderYearCalendar({
    year,
    weekLabels,
    classSessions,
    classLessons,
    calendarClosures,
    escapeHtml,
  });

  return `
    <div class="class-calendar-hero">
      <div>
        <p class="eyebrow">Class calendar</p>
        <h2>${escapeHtml(className || 'No class selected')}</h2>
        <p>${escapeHtml(academicLabel)} - ${scheduledClassDays} scheduled class days, holidays excluded</p>
      </div>
      <div class="class-calendar-controls">
        <div class="segmented">${(['month', 'year'] as ClassCalendarMode[]).map((mode) => `<button type="button" class="${classCalendarMode === mode ? 'active' : ''}" data-class-calendar-mode="${mode}">${capitalize(mode)}</button>`).join('')}</div>
        <div class="calendar-nav">
          <button class="ghost-button compact-button" type="button" data-class-calendar-shift="-1">Prev</button>
          <button class="ghost-button compact-button" type="button" data-class-calendar-today="true">Today</button>
          <button class="ghost-button compact-button" type="button" data-class-calendar-shift="1">Next</button>
        </div>
      </div>
    </div>
    <div class="class-calendar-summary">
      <div><span>Weekly rhythm</span><strong>${escapeHtml(scheduleSummary)}</strong></div>
      <div><span>Lesson plan</span><strong>${generatedLessons.length > 0 ? `${generatedLessons.length} lessons` : 'No plan linked'}</strong></div>
      <div><span>Flex days</span><strong>${generatedLessons.filter((lesson) => lesson.isBuffer).length}</strong></div>
      <div><span>Closures</span><strong>${calendarClosures.length}</strong></div>
    </div>
    <div class="class-calendar-kpis">
      <article><span>Scheduled days</span><strong>${scheduledClassDays}</strong></article>
      <article><span>Completed lessons</span><strong>${completedLessons}</strong></article>
      <article><span>Missed lessons</span><strong>${missedLessons}</strong></article>
      <article><span>Upcoming lessons</span><strong>${upcomingLessons}</strong></article>
    </div>
    ${generatedLessons.some((lesson) => lesson.isBuffer) ? '<p class="calendar-footnote">Flex days are review, reteaching, or catch-up time built into the plan automatically.</p>' : ''}
    ${classCalendarMode === 'year' ? yearCalendar : `<div class="class-calendar-month-head"><h3>${escapeHtml(monthTitle)}</h3><span>${escapeHtml(schoolName || 'School calendar')}</span></div>${monthCalendar}`}
    ${generatedLessons.some((lesson) => lesson.teachingDate < formatYmdUtc(new Date()) && lesson.status !== 'completed') ? `<button id="reschedule-missed" class="primary-button compact-button" type="button">${icon('calendar')} Reschedule missed lessons</button>` : ''}
  `;
}

export function renderDayLessonsListModal(
  date: string,
  lessons: CalendarLessonListEntry[],
  escapeHtml: (value: string) => string,
  icon: (name: string) => string,
): string {
  const rows = lessons.map((entry) => {
    const statusLabel = entry.lesson.status === 'completed' ? 'Completed' : entry.lesson.status === 'missed' ? 'Not Completed' : 'Planned';
    const viewAction = entry.lesson.detailedPlanAttached
      ? `<button class="ghost-button compact-button" type="button" data-view-lesson="${escapeHtml(entry.lesson.id)}">View Lesson Plan</button>`
      : '<span style="font-size:11px;color:#64748b">No detailed plan</span>';
    return `<li style="padding:8px 10px;margin:4px 0;background:#f8fafc;border-radius:6px;display:flex;justify-content:space-between;align-items:center;gap:8px">
      <div><strong>${escapeHtml(entry.className)}</strong>${entry.subjectName ? ` - ${escapeHtml(entry.subjectName)}` : ''}<br><span style="font-size:12px;color:#475569">${escapeHtml(entry.lesson.lessonTitle)} - ${statusLabel}</span></div>
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

export function renderDayClassScheduleModal(
  date: string,
  sessions: CalendarClassSessionView[],
  escapeHtml: (value: string) => string,
  icon: (name: string) => string,
): string {
  const rows = sessions.map((entry) => {
    const statusLabel = entry.session.completed ? 'Completed' : 'Scheduled';
    return `<li style="padding:8px 10px;margin:4px 0;background:#f8fafc;border-radius:6px;display:flex;justify-content:space-between;align-items:center;gap:8px">
      <div>
        <strong>${escapeHtml(entry.className)}</strong>${entry.subjectName ? ` - ${escapeHtml(entry.subjectName)}` : ''}<br>
        <span style="font-size:12px;color:#475569">${escapeHtml(entry.schoolName)} - ${statusLabel}</span>
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

export function renderLessonDetailModal(params: RenderLessonDetailModalParams): string {
  const {
    lesson,
    className,
    periodMinutes,
    teacherName,
    parseLessonPlan,
    escapeHtml,
    capitalize,
    icon,
  } = params;
  const plan = parseLessonPlan(lesson.lessonObjective);
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
      bodyHtml = '<p><strong>Flex day:</strong> this date is reserved for review, reteaching, or catch-up. You can generate activities for this day while keeping it marked as flex for later rescheduling.</p>';
    } else {
      bodyHtml = `<p>${escapeHtml(lesson.lessonObjective ?? 'No lesson plan details available.')}</p>`;
    }
  } else {
    const flexDayNote = lesson.isBuffer
      ? '<p><strong>Flex day:</strong> this stays marked as a flex/catch-up day, but now includes optional classroom activities you can run if no makeup lesson is needed.</p>'
      : '';
    const backwardDesignBlock = plan.backwardDesign && typeof plan.backwardDesign === 'object'
      ? `<div class="bd-section"><h4>Backward Design Template</h4><details open><summary class="label">Full structured plan</summary><pre style="white-space:pre-wrap;line-height:1.35;font-size:12px;color:#334155;background:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;padding:10px;max-height:360px;overflow:auto">${escapeHtml(JSON.stringify(plan.backwardDesign, null, 2))}</pre></details></div>`
      : '';
    bodyHtml = `
      ${flexDayNote}
      <div class="bd-section"><h4>Learning Objective</h4><p>${escapeHtml(plan.objective)}</p></div>
      <div class="bd-section"><h4>Transfer Goal</h4><p>${escapeHtml(plan.transferGoal)}</p></div>
      <div class="bd-section"><h4>Enduring Understanding</h4><p>${escapeHtml(plan.enduringUnderstanding)}</p></div>
      <div class="bd-section"><h4>Essential Vocabulary</h4><ul>${plan.vocabulary.map((value) => `<li>${escapeHtml(value)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Guiding Questions</h4><ul>${plan.essentialQuestions.map((value) => `<li>${escapeHtml(value)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Knowledge</h4><ul>${plan.knowledge.map((value) => `<li>${escapeHtml(value)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Skills</h4><ul>${plan.skills.map((value) => `<li>${escapeHtml(value)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Success Criteria</h4><ul>${plan.successCriteria.map((value) => `<li>${escapeHtml(value)}</li>`).join('')}</ul></div>
      <div class="bd-section"><h4>Assessment Evidence</h4><p>${escapeHtml(plan.assessment)}</p>${plan.performanceTask ? `<p><strong>Performance Task:</strong> ${escapeHtml(plan.performanceTask)}</p>` : ''}</div>
      <div class="bd-section"><h4>Learning Plan</h4><table class="flow-table"><thead><tr><th>Stage</th><th>Time</th><th>Activity</th></tr></thead><tbody>${plan.lessonFlow.map((step) => `<tr><td>${escapeHtml(step.phase)}</td><td>${escapeHtml(step.time)}</td><td>${escapeHtml(step.description)}</td></tr>`).join('')}</tbody></table></div>
      <div class="bd-section"><h4>Materials</h4><ul>${plan.materials.map((value) => `<li>${escapeHtml(value)}</li>`).join('')}</ul></div>
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
          <p>${className ? `${escapeHtml(className)} - ` : ''}${escapeHtml(lesson.teachingDate)} - ${escapeHtml(capitalize(lesson.weekday))} - ${periodMinutes} min - <span class="${statusClass}">${statusLabel}</span></p>
        </div>
        <button class="ghost-button compact-button modal-close" type="button">${icon('x')}</button>
      </div>
      <div class="modal-body">${bodyHtml}${statusActions}<p class="settings-detail">Teacher: ${escapeHtml(teacherName)}</p></div>
    </div>
  </div>`;
}

export function bindClassCalendarTabEvents(params: BindClassCalendarTabEventsParams): void {
  const {
    onSetMode,
    onShift,
    onToday,
    isMainCalendarPage,
    getMainCalendarSessionsByDate,
    getClassLessonsByDate,
    getClassLessonById,
    selectedClassName,
    renderLessonDetailModal,
    renderDayClassScheduleModal,
    renderDayLessonsListModal,
    bindLessonStatusButtons,
    onRescheduleMissed,
  } = params;

  document.querySelectorAll<HTMLButtonElement>('[data-class-calendar-mode]').forEach((button) => {
    button.addEventListener('click', () => {
      onSetMode(button.dataset.classCalendarMode as ClassCalendarMode);
    });
  });
  document.querySelectorAll<HTMLButtonElement>('[data-class-calendar-shift]').forEach((button) => {
    button.addEventListener('click', () => {
      onShift(Number(button.dataset.classCalendarShift ?? '0'));
    });
  });
  document.querySelector<HTMLButtonElement>('[data-class-calendar-today]')?.addEventListener('click', onToday);

  document.querySelectorAll<HTMLElement>('.month-day.has-lesson').forEach((element) => {
    element.addEventListener('click', (event) => {
      if ((event.target as HTMLElement).closest('.lesson-btn, .lesson-actions, [data-open-lesson-plan]')) return;
      const date = element.dataset.date;
      if (!date) return;
      document.getElementById('lesson-modal')?.remove();

      const isMainCalendar = isMainCalendarPage();
      const mainCalendarSessions = isMainCalendar ? getMainCalendarSessionsByDate(date) : [];
      const classLessons = getClassLessonsByDate(date);
      if (isMainCalendar && mainCalendarSessions.length === 0) return;
      if (!isMainCalendar && classLessons.length === 0) return;

      const html = isMainCalendar
        ? renderDayClassScheduleModal(date, mainCalendarSessions)
        : classLessons.length === 1
          ? renderLessonDetailModal(classLessons[0].lesson, classLessons[0].className)
          : renderDayLessonsListModal(date, classLessons);

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
      modal.querySelectorAll<HTMLElement>('[data-view-lesson]').forEach((button) => {
        button.addEventListener('click', () => {
          const lessonId = button.dataset.viewLesson;
          const match = classLessons.find((entry) => entry.lesson.id === lessonId);
          if (!match) return;
          modal.innerHTML = renderLessonDetailModal(match.lesson, match.className);
          bindLessonStatusButtons(modal);
          modal.querySelector<HTMLElement>('.modal-close')?.addEventListener('click', () => modal.remove());
          modal.addEventListener('click', (modalEvent) => {
            if (modalEvent.target === modal) modal.remove();
          });
        });
      });
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-open-lesson-plan]').forEach((button) => {
    button.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();
      const lessonId = button.dataset.openLessonPlan ?? '';
      const lesson = getClassLessonById(lessonId);
      if (!lesson) return;
      document.getElementById('lesson-modal')?.remove();
      const wrapper = document.createElement('div');
      wrapper.innerHTML = renderLessonDetailModal(lesson, selectedClassName());
      document.body.appendChild(wrapper.firstElementChild!);
      const modal = document.getElementById('lesson-modal')!;
      bindLessonStatusButtons(modal);
      modal.addEventListener('click', (modalEvent) => {
        if ((modalEvent.target as HTMLElement).closest('.modal-close') || modalEvent.target === modal) {
          modal.remove();
        }
      });
    });
  });

  bindLessonStatusButtons(document);
  document.querySelector<HTMLButtonElement>('#reschedule-missed')?.addEventListener('click', () => {
    void onRescheduleMissed();
  });
}

function renderYearCalendar(params: {
  year: number;
  weekLabels: string[];
  classSessions: SessionView[];
  classLessons: YearPlanLesson[];
  calendarClosures: CalendarClosureDay[];
  escapeHtml: (value: string) => string;
}): string {
  const { year, weekLabels, classSessions, classLessons, calendarClosures, escapeHtml } = params;
  return `<div class="year-calendar-head"><h3>${year}</h3><span>${classSessions.length} scheduled class days in this class, holidays excluded</span></div>
    <div class="year-calendar-grid">${Array.from({ length: 12 }, (_, month) => {
      const date = new Date(Date.UTC(year, month, 1));
      const monthName = date.toLocaleDateString(undefined, { month: 'long', timeZone: 'UTC' });
      const monthMarkup = renderMonthCalendar({
        year,
        month,
        compact: true,
        weekLabels,
        classSessions,
        classLessons,
        calendarClosures,
        escapeHtml,
      });
      return `<section class="year-month"><h4>${escapeHtml(monthName)}</h4>${monthMarkup}</section>`;
    }).join('')}</div>`;
}

function renderMonthCalendar(params: {
  year: number;
  month: number;
  compact: boolean;
  weekLabels: string[];
  classSessions: SessionView[];
  classLessons: YearPlanLesson[];
  calendarClosures: CalendarClosureDay[];
  escapeHtml: (value: string) => string;
}): string {
  const {
    year,
    month,
    compact,
    weekLabels,
    classSessions,
    classLessons,
    calendarClosures,
    escapeHtml,
  } = params;
  const dates = monthGridDates(year, month);
  return `<div class="month-grid ${compact ? 'mini-month-grid' : ''}">
    ${weekLabels.map((label) => `<div class="weekday-head">${label}</div>`).join('')}
    ${dates.map((date) => {
      const iso = formatYmdUtc(date);
      const inMonth = date.getUTCMonth() === month;
      const closure = calendarClosures.find((item) => item.closureDate === iso) ?? null;
      const session = closure ? null : classSessions.find((item) => item.sessionDate === iso) ?? null;
      const lesson = closure ? null : classLessons.find((item) => item.teachingDate === iso) ?? null;

      const lessonPill = lesson
        ? lesson.isBuffer
          ? '<span class="day-pill buffer-pill">Flex</span>'
          : `<span class="day-pill lesson-pill status-${escapeHtml(lesson.status)}">${escapeHtml(lesson.lessonTitle)}</span>`
        : '';
      const lessonStateBadge = lesson && !lesson.isBuffer
        ? lesson.status === 'completed'
          ? '<span class="lesson-state-badge is-complete">Complete</span>'
          : lesson.status === 'missed'
            ? '<span class="lesson-state-badge is-incomplete">Not completed</span>'
            : '<span class="lesson-state-badge is-pending">Pending</span>'
        : '';
      const lessonActions = lesson && !lesson.isBuffer && !compact
        ? `<div class="lesson-actions">
            <button class="lesson-status-chip complete ${lesson.status === 'completed' ? 'active' : ''}" data-mark-lesson="${escapeHtml(lesson.id)}" data-status="completed" title="Mark complete">Complete</button>
            <button class="lesson-status-chip incomplete ${lesson.status === 'missed' ? 'active' : ''}" data-mark-lesson="${escapeHtml(lesson.id)}" data-status="missed" title="Mark not completed">Not completed</button>
          </div>`
        : '';
      const viewPlanButton = lesson && lesson.detailedPlanAttached && !lesson.isBuffer && !compact
        ? `<button class="ghost-button compact-button" type="button" data-open-lesson-plan="${escapeHtml(lesson.id)}">View Lesson Plan</button>`
        : '';

      return `<div class="month-day ${inMonth ? '' : 'muted-day'} ${isTodayIso(iso) ? 'today' : ''} ${session ? 'has-class' : ''} ${closure ? 'has-closure' : ''} ${lesson ? 'has-lesson' : ''}" data-date="${escapeHtml(iso)}">
        <div class="day-number">${date.getUTCDate()}</div>
        ${session ? '<span class="day-pill class-pill-dot">Class</span>' : ''}
        ${lessonPill}
        ${lessonStateBadge}
        ${lessonActions}
        ${viewPlanButton}
        ${closure ? `<span class="day-pill closure-pill">${escapeHtml(closure.title ?? 'Closure')}</span>` : ''}
      </div>`;
    }).join('')}
  </div>`;
}
