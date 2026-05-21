import type { LessonExportFormat, LessonExportScope } from '../types';

type RenderLessonPlansPageParams = {
  schoolName: string;
  className: string;
  lessonCount: number;
  detailedCount: number;
  nonBufferLessonCount: number;
  scope: LessonExportScope;
  format: LessonExportFormat;
  exportScopeTitle: string;
  groupsCount: number;
  selectedCount: number;
  exportPreviewMarkup: string;
  icon: (name: string) => string;
  escapeHtml: (value: string) => string;
  emptyState: (title: string, body: string) => string;
  capitalize: (value: string) => string;
};

export function renderLessonPlansPage(params: RenderLessonPlansPageParams): string {
  const {
    schoolName,
    className,
    lessonCount,
    detailedCount,
    nonBufferLessonCount,
    scope,
    format,
    exportScopeTitle,
    groupsCount,
    selectedCount,
    exportPreviewMarkup,
    icon,
    escapeHtml,
    emptyState,
    capitalize,
  } = params;

  return `
    <section class="lesson-plans-grid">
      <article class="panel lesson-export-panel animate-fade-in-up delay-100">
        <p class="eyebrow">Lesson plans</p>
        <h2>Export teaching plans</h2>
        <p>Review generated plans, then export by lesson, week, month, or year.</p>
        <div class="lesson-kpis">
          <article><span>Total lessons</span><strong>${lessonCount}</strong></article>
          <article><span>Detailed plans</span><strong>${detailedCount}</strong></article>
          <article><span>Pending detail</span><strong>${Math.max(0, nonBufferLessonCount - detailedCount)}</strong></article>
          <article><span>Selected export</span><strong>${selectedCount}</strong></article>
        </div>
        <div class="class-calendar-summary">
          <div><span>School</span><strong>${escapeHtml(schoolName)}</strong></div>
          <div><span>Class</span><strong>${escapeHtml(className)}</strong></div>
          <div><span>Lessons</span><strong>${lessonCount}</strong></div>
          <div><span>Detailed</span><strong>${detailedCount}/${nonBufferLessonCount}</strong></div>
        </div>
        <p class="settings-detail">Use the Syllabus page to generate and attach detailed lesson plans.</p>
        <div class="segmented lesson-scope-control">${(['lesson', 'week', 'month', 'year'] as LessonExportScope[]).map((item) => `<button type="button" class="${scope === item ? 'active' : ''}" data-lesson-export-scope="${item}">${capitalize(item)}</button>`).join('')}</div>
        <div class="lesson-format-row"><span class="label">Format</span><div class="segmented">${(['html', 'md', 'txt', 'docx', 'pdf'] as LessonExportFormat[]).map((item) => `<button type="button" class="${format === item ? 'active' : ''}" data-lesson-export-format="${item}">${item.toUpperCase()}</button>`).join('')}</div></div>
      </article>
      <article class="panel animate-fade-in-up delay-200">
        <p class="eyebrow">Export</p>
        <h2>${escapeHtml(exportScopeTitle)}</h2>
        ${groupsCount > 0
          ? `<div class="lesson-export-toolbar"><label class="lesson-select-all"><input id="lesson-select-all" type="checkbox" ${selectedCount === groupsCount ? 'checked' : ''} /><span>Select all</span></label><button id="export-selected-groups" class="primary-button compact-button" type="button" ${selectedCount > 0 ? '' : 'disabled'}>${icon('upload')} Export selected (${selectedCount})</button></div>${exportPreviewMarkup}`
          : emptyState('No lesson plan', 'Upload a syllabus, review sections, and generate a class plan first.')}
      </article>
    </section>
  `;
}
