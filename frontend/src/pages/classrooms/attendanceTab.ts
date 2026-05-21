import type { AttendanceView, SessionView, Student } from '../../types';

type RenderClassroomAttendanceTabParams = {
  showAttendanceHistory: boolean;
  students: Student[];
  totalSessions: SessionView[];
  visibleSessions: SessionView[];
  attendanceRecords: AttendanceView[];
  emptyState: (title: string, body: string) => string;
  escapeHtml: (value: string) => string;
  shortDate: (value: string) => string;
  isAttendedStatus: (status?: string | null) => boolean;
  attendanceTone: (status: string) => string;
  attendanceMark: (status: string) => string;
};

type BindClassroomAttendanceEventsParams = {
  onAdvanceAttendance: (sessionId: string, studentId: string) => Promise<void>;
  onToggleHistory: () => void;
};

const ATTENDANCE_GUIDE_DISMISSED_KEY = 'edutrack.attendanceGuide.dismissed.v1';
const ATTENDANCE_GUIDE_SHOWN_SESSION_KEY = 'edutrack.attendanceGuide.shown.v1';
let attendanceGuideEscapeBound = false;

function openAttendanceGuide(): void {
  const modal = document.querySelector<HTMLElement>('#attendance-guide-modal');
  if (!modal) return;
  modal.hidden = false;
  modal.setAttribute('aria-hidden', 'false');
  document.body.classList.add('is-modal-open');
  modal.querySelector<HTMLButtonElement>('[data-attendance-guide-close]')?.focus();
}

function closeAttendanceGuide(): void {
  const modal = document.querySelector<HTMLElement>('#attendance-guide-modal');
  if (!modal) return;
  modal.hidden = true;
  modal.setAttribute('aria-hidden', 'true');
  document.body.classList.remove('is-modal-open');
  try {
    localStorage.setItem(ATTENDANCE_GUIDE_DISMISSED_KEY, '1');
  } catch { /* ignore storage failures */ }
}

function handleAttendanceGuideEscape(event: KeyboardEvent): void {
  if (event.key !== 'Escape') return;
  const modal = document.querySelector<HTMLElement>('#attendance-guide-modal');
  if (!modal || modal.hidden) return;
  closeAttendanceGuide();
}

function shouldAutoShowAttendanceGuide(): boolean {
  try {
    if (localStorage.getItem(ATTENDANCE_GUIDE_DISMISSED_KEY) === '1') return false;
    if (sessionStorage.getItem(ATTENDANCE_GUIDE_SHOWN_SESSION_KEY) === '1') return false;
    sessionStorage.setItem(ATTENDANCE_GUIDE_SHOWN_SESSION_KEY, '1');
    return true;
  } catch {
    return false;
  }
}

function attendanceStatus(
  records: AttendanceView[],
  studentId: string,
  sessionId: string,
): string {
  return records.find((record) => record.studentId === studentId && record.sessionId === sessionId)?.status ?? '';
}

function attendanceStatusLabel(status: string): string {
  if (status === 'present') return 'Present (P)';
  if (status === 'late') return 'Late (L)';
  if (status === 'absent') return 'Absent (A)';
  if (status === 'excused') return 'Excused (E)';
  if (status === 'ignore') return 'Ignore (I)';
  return 'Unmarked (-)';
}

function nextAttendanceStatusLabel(status: string): string {
  if (!status) return 'Present (P)';
  if (status === 'present') return 'Late (L)';
  if (status === 'late') return 'Absent (A)';
  if (status === 'absent') return 'Excused (E)';
  if (status === 'excused') return 'Ignore (I)';
  return 'Present (P)';
}

export function renderClassroomAttendanceTab(
  params: RenderClassroomAttendanceTabParams,
): string {
  const {
    showAttendanceHistory,
    students,
    totalSessions,
    visibleSessions,
    attendanceRecords,
    emptyState,
    escapeHtml,
    shortDate,
    isAttendedStatus,
    attendanceTone,
    attendanceMark,
  } = params;

  const presentCount = attendanceRecords.filter((record) => record.status === 'present').length;
  const lateCount = attendanceRecords.filter((record) => record.status === 'late').length;
  const absentCount = attendanceRecords.filter((record) => record.status === 'absent').length;

  let tableMarkup = '';
  if (students.length === 0) {
    tableMarkup = emptyState('No students', 'Add students before taking attendance.');
  } else if (totalSessions.length === 0) {
    tableMarkup = emptyState('No attendance dates', 'Choose class meeting days and EduTrack will create the date columns.');
  } else if (visibleSessions.length === 0) {
    tableMarkup = emptyState('No upcoming attendance', 'There are no class dates from today onward. Use Show history to view past attendance.');
  } else {
    tableMarkup = `
      <div class="attendance-sheet-wrap">
        <table class="attendance-sheet">
          <thead><tr><th class="sticky-col student-col">Student</th>${visibleSessions.map((session) => `<th class="date-col"><span>${escapeHtml(shortDate(session.sessionDate))}</span><small>${escapeHtml(session.title)}</small></th>`).join('')}<th class="sticky-total total-col">Attended total</th></tr></thead>
          <tbody>${students.map((student, index) => {
            const studentTotal = totalSessions.filter((session) => attendanceStatus(attendanceRecords, student.id, session.id) !== 'ignore').length;
            const attendedTotal = totalSessions.filter((session) => isAttendedStatus(attendanceStatus(attendanceRecords, student.id, session.id))).length;
            const percent = studentTotal > 0 ? Math.round((attendedTotal / studentTotal) * 100) : 0;
            return `<tr class="${index % 2 === 0 ? 'row-even' : 'row-odd'}" data-attendance-student-row="1" data-student-name="${escapeHtml(student.fullName.toLowerCase())}">
              <th class="sticky-col student-col" scope="row"><strong>${escapeHtml(student.fullName)}</strong><span>${escapeHtml(student.studentCode ?? student.preferredName ?? 'No code')}</span></th>
              ${visibleSessions.map((session) => {
                const status = attendanceStatus(attendanceRecords, student.id, session.id);
                const currentLabel = attendanceStatusLabel(status);
                const nextLabel = nextAttendanceStatusLabel(status);
                return `<td><button type="button" class="attendance-cell ${attendanceTone(status)}" data-attendance-cell="true" data-session-id="${escapeHtml(session.id)}" data-student-id="${escapeHtml(student.id)}" title="${escapeHtml(`${currentLabel}. Click to set ${nextLabel}.`)}" aria-label="${escapeHtml(`${student.fullName} ${session.sessionDate}. ${currentLabel}. Click to set ${nextLabel}.`)}">${escapeHtml(attendanceMark(status))}</button></td>`;
              }).join('')}
              <td class="sticky-total total-col"><strong>${percent}%</strong><span>${attendedTotal}/${studentTotal}</span></td>
            </tr>`;
          }).join('')}</tbody>
        </table>
      </div>
      <div class="attendance-legend">
        <span><i class="present"></i>Present</span><span><i class="late"></i>Late</span><span><i class="absent"></i>Absent</span>
        <span><i class="excused"></i>Excused</span><span><i class="ignore"></i>Ignore</span><span><i class="blank"></i>Unmarked</span>
      </div>
    `;
  }

  return `<section class="panel">
    <div class="panel-head">
      <div><p class="eyebrow">Attendance register</p><h2>Class attendance</h2></div>
      <div class="attendance-panel-actions">
        <div class="attendance-kpis" aria-hidden="true">
          <span>P ${presentCount}</span>
          <span>L ${lateCount}</span>
          <span>A ${absentCount}</span>
        </div>
        <input id="attendance-student-filter" type="search" class="attendance-student-filter" placeholder="Filter students..." aria-label="Filter students" />
        <button id="attendance-guide-open" class="ghost-button compact-button" type="button">Attendance codes</button>
        <button id="toggle-attendance-history" class="ghost-button" type="button">${showAttendanceHistory ? 'Hide history' : 'Show history'}</button>
      </div>
    </div>
    ${tableMarkup}
    <div class="modal-overlay" id="attendance-guide-modal" hidden aria-hidden="true" role="dialog" aria-modal="true" aria-labelledby="attendance-guide-title">
      <div class="modal-card attendance-guide-card">
        <div class="modal-header">
          <div class="attendance-guide-head">
            <p class="attendance-guide-eyebrow">Quick guide</p>
            <h3 id="attendance-guide-title">Attendance code walkthrough</h3>
            <p>Click the same attendance cell to cycle through marks.</p>
          </div>
          <button class="ghost-button compact-button" type="button" data-attendance-guide-close>Close</button>
        </div>
        <div class="modal-body">
          <p class="attendance-guide-cycle" aria-label="Attendance code cycle">P -> L -> A -> E -> I -> P</p>
          <div class="attendance-guide-code-grid">
            <article class="attendance-guide-code code-present">
              <span class="attendance-guide-code-pill">P</span>
              <div><strong>Present</strong><p>Student attended class.</p></div>
            </article>
            <article class="attendance-guide-code code-late">
              <span class="attendance-guide-code-pill">L</span>
              <div><strong>Late</strong><p>Student arrived late.</p></div>
            </article>
            <article class="attendance-guide-code code-absent">
              <span class="attendance-guide-code-pill">A</span>
              <div><strong>Absent</strong><p>Student did not attend.</p></div>
            </article>
            <article class="attendance-guide-code code-excused">
              <span class="attendance-guide-code-pill">E</span>
              <div><strong>Excused</strong><p>Absence is approved.</p></div>
            </article>
            <article class="attendance-guide-code code-ignore">
              <span class="attendance-guide-code-pill">I</span>
              <div><strong>Ignore</strong><p>Exclude this date from attendance percentage.</p></div>
            </article>
          </div>
          <p class="attendance-guide-tip">Tip: Use <strong>I</strong> for holidays, special events, or non-teaching days kept in history.</p>
        </div>
      </div>
    </div>
  </section>`;
}

export function bindClassroomAttendanceEvents(
  params: BindClassroomAttendanceEventsParams,
): void {
  const { onAdvanceAttendance, onToggleHistory } = params;

  document.querySelectorAll<HTMLButtonElement>('[data-attendance-cell]').forEach((button) => {
    button.addEventListener('click', async () => {
      const sessionId = button.dataset.sessionId ?? '';
      const studentId = button.dataset.studentId ?? '';
      if (!sessionId || !studentId) return;
      await onAdvanceAttendance(sessionId, studentId);
    });
  });

  document
    .querySelector<HTMLButtonElement>('#toggle-attendance-history')
    ?.addEventListener('click', onToggleHistory);

  document
    .querySelector<HTMLButtonElement>('#attendance-guide-open')
    ?.addEventListener('click', openAttendanceGuide);

  document
    .querySelector<HTMLInputElement>('#attendance-student-filter')
    ?.addEventListener('input', (event) => {
      const query = ((event.currentTarget as HTMLInputElement).value || '').trim().toLowerCase();
      document.querySelectorAll<HTMLElement>('[data-attendance-student-row="1"]').forEach((row) => {
        const name = row.dataset.studentName ?? '';
        row.hidden = query.length > 0 && !name.includes(query);
      });
    });

  document
    .querySelectorAll<HTMLButtonElement>('[data-attendance-guide-close]')
    .forEach((button) => button.addEventListener('click', closeAttendanceGuide));

  document
    .querySelector<HTMLElement>('#attendance-guide-modal')
    ?.addEventListener('click', (event) => {
      if (event.target === event.currentTarget) closeAttendanceGuide();
    });

  if (!attendanceGuideEscapeBound) {
    document.addEventListener('keydown', handleAttendanceGuideEscape);
    attendanceGuideEscapeBound = true;
  }

  if (shouldAutoShowAttendanceGuide()) openAttendanceGuide();
}
