import type { ClassView, Student, StudentReportStatus } from '../types';

type RenderReportsPageParams = {
  activeClassName: string | null;
  classes: ClassView[];
  selectedClassId: string;
  students: Student[];
  reportStatuses: StudentReportStatus[];
  reportInstructions: string;
  reportWordCount: number;
  icon: (name: string) => string;
  escapeHtml: (value: string) => string;
};

type BindReportsPageEventsParams = {
  onSelectClass: (classId: string) => Promise<void>;
  onInstructionsInput: (value: string) => void;
  onWordCountChange: (value: number) => void;
  onGenerateAllReports: () => Promise<void>;
  onGenerateStudentReport: (studentId: string) => Promise<void>;
  onViewReport: (reportId: string) => Promise<void>;
  onExportReport: (reportId: string) => Promise<void>;
};

export function renderReportsPage(params: RenderReportsPageParams): string {
  const {
    activeClassName,
    classes,
    selectedClassId,
    students,
    reportStatuses,
    reportInstructions,
    reportWordCount,
    icon,
    escapeHtml,
  } = params;

  const hasClass = Boolean(activeClassName);
  const generatedCount = reportStatuses.filter((r) => r.reportExists).length;
  const missingCount = Math.max(0, students.length - generatedCount);

  return `
    <section class="reports-layout">
      <article class="panel reports-hero">
        <div class="panel-head">
          <div>
            <p class="eyebrow">Student reports</p>
            <h2>Power report workspace</h2>
            <p>Pick a class, set instructions, then generate high-quality reports in bulk or per student.</p>
          </div>
          <div class="reports-head-actions">
            <label class="reports-class-select">
              <span class="label">Class</span>
              <select id="reports-class-select">
                <option value="">Select class</option>
                ${classes.map((klass) => `<option value="${escapeHtml(klass.id)}" ${selectedClassId === klass.id ? 'selected' : ''}>${escapeHtml(klass.name)}</option>`).join('')}
              </select>
            </label>
            <button id="generate-all-reports" class="primary-button" type="button" ${hasClass && students.length > 0 ? '' : 'disabled'}>${icon('book')} Generate reports for all students</button>
          </div>
        </div>

        <div class="reports-kpis">
          <article class="reports-kpi"><span>Total students</span><strong>${students.length}</strong></article>
          <article class="reports-kpi"><span>Reports ready</span><strong>${generatedCount}</strong></article>
          <article class="reports-kpi"><span>Not generated</span><strong>${missingCount}</strong></article>
        </div>

        <div class="reports-input-grid">
          <label>
            <span class="label">Extra instructions</span>
            <textarea id="report-instructions" rows="3" placeholder="Optional: focus areas, tone, strengths/next steps, parent-facing style...">${escapeHtml(reportInstructions)}</textarea>
          </label>
          <label class="reports-word-count">
            <span class="label">Word count</span>
            <input id="report-word-count" type="number" min="80" max="2000" value="${reportWordCount}" />
          </label>
        </div>
      </article>

      ${hasClass ? `
      <section class="panel reports-students-panel">
        <div class="reports-students-head">
          <div><p class="eyebrow">${escapeHtml(activeClassName ?? '')}</p><h3>Class list</h3></div>
          <input id="reports-student-filter" type="search" placeholder="Filter students..." aria-label="Filter students" />
        </div>
        <div class="reports-grid">
          ${students.map((student) => {
            const status = reportStatuses.find((r) => r.studentId === student.id);
            const hasReport = status?.reportExists ?? false;
            const reportId = status?.reportId ?? '';
            return `
              <article class="report-student-card" data-student-card="1" data-student-name="${escapeHtml(student.fullName.toLowerCase())}">
                <div class="report-student-head">
                  <strong>${escapeHtml(student.fullName)}</strong>
                  ${hasReport ? '<span class="test-row-status completed">Ready</span>' : '<span class="test-row-status pending">Missing</span>'}
                </div>
                <div class="report-actions">
                  <button type="button" class="ghost-button compact-button" data-generate-student-report="${escapeHtml(student.id)}">${hasReport ? 'Regenerate' : 'Generate'}</button>
                  ${hasReport ? `<button type="button" class="ghost-button compact-button" data-view-report="${escapeHtml(reportId)}">View</button>
                  <button type="button" class="ghost-button compact-button" data-export-report="${escapeHtml(reportId)}">${icon('upload')} Export</button>` : ''}
                </div>
              </article>
            `;
          }).join('')}
        </div>
      </section>
      ` : ''}
    </section>
  `;
}

export function bindReportsPageEvents(params: BindReportsPageEventsParams): void {
  const {
    onSelectClass,
    onInstructionsInput,
    onWordCountChange,
    onGenerateAllReports,
    onGenerateStudentReport,
    onViewReport,
    onExportReport,
  } = params;

  document.querySelector<HTMLSelectElement>('#reports-class-select')?.addEventListener('change', async (event) => {
    await onSelectClass((event.currentTarget as HTMLSelectElement).value);
  });

  document.querySelector<HTMLTextAreaElement>('#report-instructions')?.addEventListener('input', (event) => {
    onInstructionsInput((event.currentTarget as HTMLTextAreaElement).value);
  });

  document.querySelector<HTMLInputElement>('#report-word-count')?.addEventListener('change', (event) => {
    onWordCountChange(Number((event.currentTarget as HTMLInputElement).value) || 300);
  });

  document.querySelector<HTMLButtonElement>('#generate-all-reports')?.addEventListener('click', async () => {
    await onGenerateAllReports();
  });

  document.querySelectorAll<HTMLButtonElement>('[data-generate-student-report]').forEach((button) => {
    button.addEventListener('click', async () => {
      const studentId = button.dataset.generateStudentReport ?? '';
      if (!studentId) return;
      await onGenerateStudentReport(studentId);
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-view-report]').forEach((button) => {
    button.addEventListener('click', async () => {
      const reportId = button.dataset.viewReport ?? '';
      if (!reportId) return;
      await onViewReport(reportId);
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-export-report]').forEach((button) => {
    button.addEventListener('click', async () => {
      const reportId = button.dataset.exportReport ?? '';
      if (!reportId) return;
      await onExportReport(reportId);
    });
  });

  document.querySelector<HTMLInputElement>('#reports-student-filter')?.addEventListener('input', (event) => {
    const query = ((event.currentTarget as HTMLInputElement).value || '').trim().toLowerCase();
    document.querySelectorAll<HTMLElement>('[data-student-card="1"]').forEach((card) => {
      const name = card.dataset.studentName ?? '';
      card.hidden = query.length > 0 && !name.includes(query);
    });
  });
}
