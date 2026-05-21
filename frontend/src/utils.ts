import type {
  Page, LessonExportFormat, ClassView, Level, LessonPlanJson,
} from './types';

export function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> | null {
  const tauri = window.__TAURI__;
  const invoker = tauri?.core?.invoke ?? tauri?.invoke;
  if (!invoker) {
    console.warn('Tauri backend unavailable for command:', command);
    return null;
  }
  return invoker<T>(command, args);
}

export function icon(name: string) {
  const paths: Record<string, string> = {
    layout: '<path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>',
    users: '<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/>',
    book: '<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>',
    calendar: '<rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/>',
    cpu: '<rect x="4" y="4" width="16" height="16" rx="2" ry="2"/><rect x="9" y="9" width="6" height="6"/><line x1="9" y1="1" x2="9" y2="4"/><line x1="15" y1="1" x2="15" y2="4"/><line x1="9" y1="20" x2="9" y2="23"/><line x1="15" y1="20" x2="15" y2="23"/><line x1="20" y1="9" x2="23" y2="9"/><line x1="20" y1="14" x2="23" y2="14"/><line x1="1" y1="9" x2="4" y2="9"/><line x1="1" y1="14" x2="4" y2="14"/>',
    settings: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>',
    bot: '<path d="M12 8a6 6 0 1 0 6 6"/><path d="M8 14c-1.5 0-3-1-3-3"/><path d="M16 14c1.5 0 3-1 3-3"/><path d="M12 12v-2"/><rect x="8" y="4" width="8" height="4" rx="1"/>',
    'check-circle': '<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/>',
    'alert-circle': '<circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>',
    upload: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/>',
    x: '<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>',
  };
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths[name] ?? paths.layout}</svg>`;
}

export function escapeHtml(value: string) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

export function emptyState(title: string, body: string) {
  return `<div class="empty-state"><strong>${title}</strong><p>${body}</p></div>`;
}

export function emptyOrList(items: string[], emptyText: string) {
  if (items.length === 0) {
    return emptyState('Empty', emptyText);
  }
  return `<ul class="rich-list">${items.map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ul>`;
}

export function capitalize(value: string) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

export function getGreeting() {
  const hour = new Date().getHours();
  if (hour < 12) return 'Good morning';
  if (hour < 18) return 'Good afternoon';
  return 'Good evening';
}

export function pageTitle(page: Page) {
  const titles: Record<Page, string> = {
    overview: 'Welcome back',
    classrooms: 'Classrooms',
    syllabus: 'Syllabus planning',
    lessonPlans: 'Lesson plans',
    calendar: 'Calendar',
    agents: 'AI agents',
    tests: 'Test Authoring & Assessments',
    reports: 'Student Reports',
    settings: 'Settings',
  };
  return titles[page];
}

export function parseYmdUtc(value: string) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) {
    return null;
  }
  return new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])));
}

export function formatYmdUtc(date: Date) {
  const year = date.getUTCFullYear();
  const month = `${date.getUTCMonth() + 1}`.padStart(2, '0');
  const day = `${date.getUTCDate()}`.padStart(2, '0');
  return `${year}-${month}-${day}`;
}

export function shortDate(value: string) {
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export function formatDisplayDate(value: string) {
  const date = parseYmdUtc(value);
  if (!date) {
    return value;
  }
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric', timeZone: 'UTC' });
}

export function isTodayIso(value: string) {
  return value === formatYmdUtc(new Date());
}

export function currentMonthKey() {
  const now = new Date();
  return `${now.getFullYear()}-${`${now.getMonth() + 1}`.padStart(2, '0')}`;
}

export function academicYearWindow() {
  const now = new Date();
  const startYear = now.getMonth() >= 6 ? now.getFullYear() : now.getFullYear() - 1;
  return {
    start: `${startYear}-08-01`,
    end: `${startYear + 1}-06-30`,
    label: `${startYear}-${startYear + 1}`,
  };
}

export function browserTimezone() {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
}

export function weekdayToJsDay(weekday: string) {
  const key = weekday.toLowerCase();
  if (key === 'sunday' || key === 'sun') return 0;
  if (key === 'monday' || key === 'mon') return 1;
  if (key === 'tuesday' || key === 'tue') return 2;
  if (key === 'wednesday' || key === 'wed') return 3;
  if (key === 'thursday' || key === 'thu') return 4;
  if (key === 'friday' || key === 'fri') return 5;
  if (key === 'saturday' || key === 'sat') return 6;
  return -1;
}

export function countryName(code?: string | null) {
  if (!code) {
    return null;
  }
  try {
    return new Intl.DisplayNames([navigator.language || 'en'], { type: 'region' }).of(code) ?? code;
  } catch {
    return code;
  }
}

export function countryFlag(code: string) {
  if (!/^[A-Z]{2}$/.test(code)) {
    return '';
  }
  return String.fromCodePoint(...code.split('').map((letter) => 127397 + letter.charCodeAt(0)));
}

export const countryCodes = [
  'AF', 'AX', 'AL', 'DZ', 'AS', 'AD', 'AO', 'AI', 'AQ', 'AG', 'AR', 'AM', 'AW', 'AU', 'AT', 'AZ',
  'BS', 'BH', 'BD', 'BB', 'BY', 'BE', 'BZ', 'BJ', 'BM', 'BT', 'BO', 'BQ', 'BA', 'BW', 'BV', 'BR',
  'IO', 'BN', 'BG', 'BF', 'BI', 'CV', 'KH', 'CM', 'CA', 'KY', 'CF', 'TD', 'CL', 'CN', 'CX', 'CC',
  'CO', 'KM', 'CG', 'CD', 'CK', 'CR', 'CI', 'HR', 'CU', 'CW', 'CY', 'CZ', 'DK', 'DJ', 'DM', 'DO',
  'EC', 'EG', 'SV', 'GQ', 'ER', 'EE', 'SZ', 'ET', 'FK', 'FO', 'FJ', 'FI', 'FR', 'GF', 'PF', 'TF',
  'GA', 'GM', 'GE', 'DE', 'GH', 'GI', 'GR', 'GL', 'GD', 'GP', 'GU', 'GT', 'GG', 'GN', 'GW', 'GY',
  'HT', 'HM', 'VA', 'HN', 'HK', 'HU', 'IS', 'IN', 'ID', 'IR', 'IQ', 'IE', 'IM', 'IL', 'IT', 'JM',
  'JP', 'JE', 'JO', 'KZ', 'KE', 'KI', 'KP', 'KR', 'KW', 'KG', 'LA', 'LV', 'LB', 'LS', 'LR', 'LY',
  'LI', 'LT', 'LU', 'MO', 'MG', 'MW', 'MY', 'MV', 'ML', 'MT', 'MH', 'MQ', 'MR', 'MU', 'YT', 'MX',
  'FM', 'MD', 'MC', 'MN', 'ME', 'MS', 'MA', 'MZ', 'MM', 'NA', 'NR', 'NP', 'NL', 'NC', 'NZ', 'NI',
  'NE', 'NG', 'NU', 'NF', 'MK', 'MP', 'NO', 'OM', 'PK', 'PW', 'PS', 'PA', 'PG', 'PY', 'PE', 'PH',
  'PN', 'PL', 'PT', 'PR', 'QA', 'RE', 'RO', 'RU', 'RW', 'BL', 'SH', 'KN', 'LC', 'MF', 'PM', 'VC',
  'WS', 'SM', 'ST', 'SA', 'SN', 'RS', 'SC', 'SL', 'SG', 'SX', 'SK', 'SI', 'SB', 'SO', 'ZA', 'GS',
  'SS', 'ES', 'LK', 'SD', 'SR', 'SJ', 'SE', 'CH', 'SY', 'TW', 'TJ', 'TZ', 'TH', 'TL', 'TG', 'TK',
  'TO', 'TT', 'TN', 'TR', 'TM', 'TC', 'TV', 'UG', 'UA', 'AE', 'GB', 'US', 'UM', 'UY', 'UZ', 'VU',
  'VE', 'VN', 'VG', 'VI', 'WF', 'EH', 'YE', 'ZM', 'ZW',
];

export function countryOptions() {
  return countryCodes
    .map((code) => ({ code, name: countryName(code) ?? code }))
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((c) => `<option value="${c.code}">${countryFlag(c.code)} ${escapeHtml(c.name)}</option>`)
    .join('');
}

export function academicYearOptions() {
  const current = academicYearWindow();
  const startYear = Number(current.start.slice(0, 4));
  return Array.from({ length: 6 }, (_, index) => {
    const year = startYear + index;
    const label = `${year}-${year + 1}`;
    return `<option value="${label}" ${label === current.label ? 'selected' : ''}>${label}</option>`;
  }).join('');
}

export const weekdays = [
  { value: 'monday', label: 'Mon' },
  { value: 'tuesday', label: 'Tue' },
  { value: 'wednesday', label: 'Wed' },
  { value: 'thursday', label: 'Thu' },
  { value: 'friday', label: 'Fri' },
  { value: 'saturday', label: 'Sat' },
  { value: 'sunday', label: 'Sun' },
];

export const weekLabels = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

export function downloadTextFile(filename: string, content: string, mimeType = 'text/html') {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

export function readImageAsDataUrl(file: File) {
  return new Promise<string>((resolve, reject) => {
    if (!file.type.startsWith('image/')) {
      reject(new Error('Choose an image file.'));
      return;
    }
    if (file.size > 2_000_000) {
      reject(new Error('Choose an image smaller than 2 MB.'));
      return;
    }
    const reader = new FileReader();
    reader.addEventListener('load', () => resolve(String(reader.result ?? '')));
    reader.addEventListener('error', () => reject(new Error('Could not read the image file.')));
    reader.readAsDataURL(file);
  });
}

export function showToast(message: string) {
  const container = document.getElementById('toast-container');
  if (!container) return;
  const toast = document.createElement('div');
  toast.className = 'notice notice-ok animate-fade-in-up';
  toast.innerHTML = `
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="20" height="20"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
    <span>${escapeHtml(message)}</span>
  `;
  container.appendChild(toast);
  setTimeout(() => {
    toast.style.opacity = '0';
    toast.style.transform = 'translateY(10px)';
    toast.style.transition = 'all 0.3s ease';
    setTimeout(() => toast.remove(), 300);
  }, 3000);
}

export function classDateRange(klass: ClassView, level: Level) {
  return {
    start: klass.startDate || level.academicYearStart,
    end: klass.endDate || level.academicYearEnd,
  };
}

export function monthGridDates(year: number, month: number) {
  const first = new Date(Date.UTC(year, month, 1));
  const mondayOffset = (first.getUTCDay() + 6) % 7;
  const start = new Date(first.getTime());
  start.setUTCDate(first.getUTCDate() - mondayOffset);
  return Array.from({ length: 42 }, (_, index) => {
    const date = new Date(start.getTime());
    date.setUTCDate(start.getUTCDate() + index);
    return date;
  });
}

export function weekKey(date: Date) {
  const monday = new Date(date.getTime());
  monday.setUTCDate(monday.getUTCDate() - ((monday.getUTCDay() + 6) % 7));
  return formatYmdUtc(monday);
}

export function parseLessonPlan(objective: string | null | undefined): LessonPlanJson | null {
  if (!objective) return null;
  try {
    const parsed = JSON.parse(objective);
    if (typeof parsed !== 'object' || parsed === null) return null;
    if (parsed.v !== "1") return null;
    if (typeof parsed.objective !== 'string' || !parsed.objective) return null;
    if (!Array.isArray(parsed.essentialQuestions)) return null;
    if (!Array.isArray(parsed.knowledge)) return null;
    if (!Array.isArray(parsed.skills)) return null;
    if (!Array.isArray(parsed.successCriteria)) return null;
    if (!Array.isArray(parsed.materials)) return null;
    if (!Array.isArray(parsed.vocabulary)) return null;
    if (!Array.isArray(parsed.lessonFlow)) return null;
    return parsed as LessonPlanJson;
  } catch {
    return null;
  }
}

export function downloadExport(filename: string, content: string, mime: string, ext: string, format: LessonExportFormat) {
  if (format === 'pdf') {
    const container = document.createElement('div');
    container.style.position = 'fixed';
    container.style.left = '-9999px';
    container.style.top = '0';
    container.innerHTML = content;
    document.body.appendChild(container);
    (async () => {
      try {
        const html2pdf = (await import('html2pdf.js')).default;
        await html2pdf().set({
          margin: [0.5, 0.5, 0.5, 0.5],
          filename: `${filename}.pdf`,
          image: { type: 'jpeg', quality: 0.98 },
          html2canvas: { scale: 2, letterRendering: true, useCORS: true },
          jsPDF: { unit: 'in', format: 'letter', orientation: 'portrait' },
        }).from(container).save();
      } catch (err) {
        console.warn('PDF export failed, falling back to HTML', err);
        downloadTextFile(`${filename}.html`, content, 'text/html');
      } finally {
        document.body.removeChild(container);
      }
    })().catch((err) => console.warn('PDF export error:', err));
    return;
  }
  downloadTextFile(`${filename}.${ext}`, content, mime);
}
