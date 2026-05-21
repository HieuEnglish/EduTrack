import type { CurriculumUnit, LessonExportScope, YearPlanLesson } from '../types';
import { formatYmdUtc, parseYmdUtc, shortDate, weekKey } from '../utils';

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

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

export function extractUnitTimelineLabel(unit: CurriculumUnit): string | null {
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

export function buildUnitAssignedRangeLabel(params: {
  unit: CurriculumUnit;
  nonBufferLessons: YearPlanLesson[];
  formatDisplayDate: (value: string) => string;
}): string {
  const { unit, nonBufferLessons, formatDisplayDate } = params;
  const scheduled = nonBufferLessons.filter((lesson) => lesson.curriculumUnitId === unit.id);
  if (scheduled.length > 0) {
    const sorted = [...scheduled].sort((a, b) => a.teachingDate.localeCompare(b.teachingDate));
    const first = sorted[0];
    const last = sorted[sorted.length - 1];
    if (first.teachingDate === last.teachingDate) return formatDisplayDate(first.teachingDate);
    return `${formatDisplayDate(first.teachingDate)} to ${formatDisplayDate(last.teachingDate)}`;
  }
  return extractUnitTimelineLabel(unit) ?? 'Timeline not detected in source';
}

export function buildDetailedPlanningContext(lessons: YearPlanLesson[]): string {
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

export function exportScopeTitle(scope: LessonExportScope): string {
  if (scope === 'lesson') return 'By lesson';
  if (scope === 'week') return 'By week';
  if (scope === 'month') return 'By month';
  return 'Full year';
}

export function groupLessonsForExport(lessons: YearPlanLesson[], scope: LessonExportScope): Array<{ key: string; title: string; lessons: YearPlanLesson[] }> {
  if (scope === 'lesson') {
    return lessons.map((lesson) => ({ key: lesson.teachingDate, title: `${lesson.teachingDate} - ${lesson.lessonTitle}`, lessons: [lesson] }));
  }
  const groups = new Map<string, YearPlanLesson[]>();
  for (const lesson of lessons) {
    const date = parseYmdUtc(lesson.teachingDate);
    const key = scope === 'week'
      ? weekKey(date ?? new Date(`${lesson.teachingDate}T00:00:00`))
      : scope === 'month'
        ? lesson.teachingDate.slice(0, 7)
        : lesson.teachingDate.slice(0, 4);
    groups.set(key, [...(groups.get(key) ?? []), lesson]);
  }
  return Array.from(groups.entries()).map(([key, groupedLessons]) => ({
    key,
    title: exportGroupTitle(key, scope),
    lessons: groupedLessons,
  }));
}

export function splitUnitAndTopic(lessonTitle: string): { unit: string; topic: string } {
  const [unitPart, ...topicParts] = lessonTitle.split(':');
  const unit = unitPart.trim();
  const topic = topicParts.join(':').trim();
  return { unit, topic };
}

export function previewUnitTags(lessons: YearPlanLesson[], limit = 3): string[] {
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

export function scopedPeriodLabel(group: { key: string; title: string }, scope: LessonExportScope): string {
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

export function lessonPreviewSummary(
  group: { key: string; title: string; lessons: YearPlanLesson[] },
  scope: LessonExportScope,
): string {
  if (scope === 'lesson') return 'Single lesson export group';
  return group.lessons.slice(0, 3).map((lesson) => lesson.lessonTitle).join(', ');
}

function exportGroupTitle(key: string, scope: LessonExportScope): string {
  if (scope === 'week') return `Week of ${key}`;
  if (scope === 'month') {
    const date = parseYmdUtc(`${key}-01`);
    return date ? date.toLocaleDateString(undefined, { month: 'long', year: 'numeric', timeZone: 'UTC' }) : key;
  }
  if (scope === 'year') return key;
  return key;
}
