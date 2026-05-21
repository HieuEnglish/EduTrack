import type { SessionView } from '../types';

type DateRange = { start: string; end: string };

export function summarizeAttendance(sessions: SessionView[]): string {
  if (sessions.length === 0) return 'No data';
  const completed = sessions.filter((session) => session.completed).length;
  return `${completed}/${sessions.length}`;
}

export function filterActiveClassSessions(params: {
  sessions: SessionView[];
  closureDates: Set<string>;
  dateRange?: DateRange | null;
}): SessionView[] {
  const { sessions, closureDates, dateRange } = params;
  return sessions.filter((session) => {
    if (closureDates.has(session.sessionDate)) return false;
    if (!dateRange) return true;
    return session.sessionDate >= dateRange.start && session.sessionDate <= dateRange.end;
  });
}

export function sortSessionsForDisplay(params: {
  sessions: SessionView[];
  showHistory: boolean;
  todayIso: string;
}): SessionView[] {
  const { sessions, showHistory, todayIso } = params;
  return sessions
    .filter((session) => showHistory || session.sessionDate >= todayIso)
    .sort((a, b) => a.sessionDate.localeCompare(b.sessionDate));
}

export function isAttendedStatus(status?: string | null): boolean {
  return status === 'present' || status === 'late';
}

export function attendanceTone(status: string): string {
  if (status === 'present') return 'is-present';
  if (status === 'late') return 'is-late';
  if (status === 'absent') return 'is-absent';
  if (status === 'excused') return 'is-excused';
  if (status === 'ignore') return 'is-ignore';
  return 'is-blank';
}

export function attendanceMark(status: string): string {
  if (status === 'present') return 'P';
  if (status === 'late') return 'L';
  if (status === 'absent') return 'A';
  if (status === 'excused') return 'E';
  if (status === 'ignore') return 'I';
  return '-';
}

export function nextAttendanceStatus(status: string): string {
  if (!status) return 'present';
  if (status === 'present') return 'late';
  if (status === 'late') return 'absent';
  if (status === 'absent') return 'excused';
  if (status === 'excused') return 'ignore';
  return 'present';
}
