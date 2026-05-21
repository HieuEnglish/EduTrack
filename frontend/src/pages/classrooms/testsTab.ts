import type { Student, StudentSubmission, TestTemplate } from '../../types';

type RenderClassroomTestsTabParams = {
  tests: TestTemplate[];
  students: Student[];
  classroomTestSubmissions: Record<string, StudentSubmission[]>;
  matrixStatus: Record<string, 'idle' | 'uploaded' | 'grading' | 'graded' | 'failed'>;
  matrixLastGradedAt: Record<string, string>;
  gradeAllBusy: boolean;
  compactMode: boolean;
  emptyState: (title: string, body: string) => string;
  escapeHtml: (value: string) => string;
};

type BindClassroomMatrixUploadEventsParams = {
  onUploadMatrix: (assessmentId: string, studentId: string, file: File) => Promise<void>;
  onUploadMatrixLink: (assessmentId: string, studentId: string, url: string) => Promise<void>;
  onGradeMatrix: (assessmentId: string, studentId: string) => Promise<void>;
  onGradeAllMatrix: (onlyUngraded: boolean) => Promise<void>;
  onGradeFailedMatrix: () => Promise<void>;
  onToggleMatrixDensity: () => void;
};

function classroomSubmissionFor(
  classroomTestSubmissions: Record<string, StudentSubmission[]>,
  testId: string,
  studentId: string,
): StudentSubmission | null {
  const rows = classroomTestSubmissions[testId] ?? [];
  return rows.find((row) => row.studentId === studentId) ?? null;
}

export function renderClassroomTestsTab(params: RenderClassroomTestsTabParams): string {
  const {
    tests,
    students,
    classroomTestSubmissions,
    matrixStatus,
    matrixLastGradedAt,
    gradeAllBusy,
    compactMode,
    emptyState,
    escapeHtml,
  } = params;

  if (tests.length === 0) {
    return `<section class="panel">${emptyState('No tests created', 'Create tests in the main Tests page, then upload student files here.')}</section>`;
  }
  if (students.length === 0) {
    return `<section class="panel">${emptyState('No students in class', 'Add students first, then upload test submissions in this matrix.')}</section>`;
  }
  return `
    <section class="panel classroom-tests-panel ${compactMode ? 'compact' : ''}">
      <div class="panel-head">
        <div><p class="eyebrow">Classroom marking</p><h2>Student uploads by test</h2></div>
        <div class="classroom-tests-head-actions">
          <span>${students.length} students · ${tests.length} tests</span>
          <button type="button" class="ghost-button compact-button" data-matrix-density-toggle="1">${compactMode ? 'Comfort spacing' : 'Compact spacing'}</button>
          <button type="button" class="ghost-button compact-button has-tooltip" data-grade-all-matrix="ungraded" data-tooltip="Grades only submissions that have a file but no score yet" aria-label="Grade only ungraded submissions" ${gradeAllBusy ? 'disabled' : ''}>${gradeAllBusy ? 'Grading...' : 'Grade ungraded only'}</button>
          <button type="button" class="ghost-button compact-button has-tooltip" data-grade-failed-matrix="1" data-tooltip="Retry grading only cells that previously failed" aria-label="Retry failed grading" ${gradeAllBusy ? 'disabled' : ''}>${gradeAllBusy ? 'Grading...' : 'Retry failed'}</button>
          <button type="button" class="primary-button compact-button has-tooltip" data-grade-all-matrix="all" data-tooltip="Grades every submitted file, including already scored ones" aria-label="Grade all submitted tests" ${gradeAllBusy ? 'disabled' : ''}>${gradeAllBusy ? 'Grading...' : 'Grade all submitted'}</button>
        </div>
      </div>
      <div class="classroom-tests-matrix-wrap">
        <table class="classroom-tests-matrix">
          <thead>
            <tr>
              <th>Student</th>
              ${tests.map((test) => `<th>${escapeHtml(test.title)}</th>`).join('')}
            </tr>
          </thead>
          <tbody>
            ${students.map((student) => `
              <tr>
                <th>${escapeHtml(student.fullName)}</th>
                ${tests.map((test) => {
                  const submission = classroomSubmissionFor(classroomTestSubmissions, test.id, student.id);
                  const fileLabel = submission?.fileName ? `<span class="submission-file">${escapeHtml(submission.fileName)}</span>` : '<span class="submission-file empty">No file</span>';
                  const scoreLabel = submission?.scoreValue != null ? `<span class="test-row-status completed">Scored ${submission.scoreValue}</span>` : '<span class="test-row-status pending">Pending</span>';
                  const hasFile = Boolean(submission?.fileName);
                  const statusKey = `${test.id}:${student.id}`;
                  const status = matrixStatus[statusKey] ?? (submission?.scoreValue != null ? 'graded' : hasFile ? 'uploaded' : 'idle');
                  const lastGradedAt = matrixLastGradedAt[statusKey] ?? '';
                  const statusLabel = status === 'grading'
                    ? 'Grading...'
                    : status === 'graded'
                      ? 'Graded'
                      : status === 'failed'
                        ? 'Failed'
                        : status === 'uploaded'
                          ? 'Uploaded'
                          : 'Not submitted';
                  return `<td>
                    <div class="classroom-test-cell">
                      ${fileLabel}
                      ${scoreLabel}
                      <span class="matrix-status-chip ${status}">${statusLabel}</span>
                      ${lastGradedAt ? `<span class="matrix-graded-at">Last graded ${escapeHtml(lastGradedAt)}</span>` : ''}
                      <label class="compact-button ghost-button classroom-upload-button">
                        Upload test
                        <input type="file" data-upload-matrix="1" data-matrix-assessment="${escapeHtml(test.id)}" data-matrix-student="${escapeHtml(student.id)}" accept=".pdf,.doc,.docx,.txt,.md,.csv,.json,.xml,.html,.htm,image/*,audio/*" />
                      </label>
                      <div class="classroom-drop-zone" data-drop-zone="1" data-matrix-assessment="${escapeHtml(test.id)}" data-matrix-student="${escapeHtml(student.id)}">Drag & drop file</div>
                      <div class="classroom-link-upload">
                        <input type="url" data-upload-link-input="1" data-matrix-assessment="${escapeHtml(test.id)}" data-matrix-student="${escapeHtml(student.id)}" placeholder="https://..." />
                        <button type="button" class="compact-button ghost-button" data-upload-link-btn="1" data-matrix-assessment="${escapeHtml(test.id)}" data-matrix-student="${escapeHtml(student.id)}">Link</button>
                      </div>
                      ${hasFile ? `<button type="button" class="compact-button primary-button" data-grade-matrix="1" data-matrix-assessment="${escapeHtml(test.id)}" data-matrix-student="${escapeHtml(student.id)}" ${status === 'grading' ? 'disabled' : ''}>${status === 'grading' ? 'Grading...' : 'Grade'}</button>` : ''}
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

export function bindClassroomMatrixUploadEvents(params: BindClassroomMatrixUploadEventsParams): void {
  const { onUploadMatrix, onUploadMatrixLink, onGradeMatrix, onGradeAllMatrix, onGradeFailedMatrix, onToggleMatrixDensity } = params;

  const getCellMeta = (element: HTMLElement) => ({
    assessmentId: element.dataset.matrixAssessment ?? '',
    studentId: element.dataset.matrixStudent ?? '',
  });

  document.querySelectorAll<HTMLInputElement>('[data-upload-matrix]').forEach((input) => {
    input.addEventListener('change', async (event) => {
      const fileInput = event.currentTarget as HTMLInputElement;
      const { assessmentId, studentId } = getCellMeta(fileInput);
      const file = fileInput.files?.[0];
      if (!assessmentId || !studentId || !file) return;
      await onUploadMatrix(assessmentId, studentId, file);
    });
  });

  document.querySelectorAll<HTMLElement>('[data-drop-zone]').forEach((zone) => {
    zone.addEventListener('dragover', (event) => {
      event.preventDefault();
      zone.classList.add('drag-over');
    });
    zone.addEventListener('dragleave', () => {
      zone.classList.remove('drag-over');
    });
    zone.addEventListener('drop', async (event) => {
      event.preventDefault();
      zone.classList.remove('drag-over');
      const { assessmentId, studentId } = getCellMeta(zone);
      const file = event.dataTransfer?.files?.[0];
      if (!assessmentId || !studentId || !file) return;
      await onUploadMatrix(assessmentId, studentId, file);
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-upload-link-btn]').forEach((button) => {
    button.addEventListener('click', async () => {
      const { assessmentId, studentId } = getCellMeta(button);
      if (!assessmentId || !studentId) return;
      const input = document.querySelector<HTMLInputElement>(
        `[data-upload-link-input][data-matrix-assessment="${assessmentId}"][data-matrix-student="${studentId}"]`,
      );
      const url = input?.value?.trim() ?? '';
      if (!url) return;
      await onUploadMatrixLink(assessmentId, studentId, url);
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-grade-all-matrix]').forEach((button) => {
    button.addEventListener('click', async () => {
      const mode = button.dataset.gradeAllMatrix ?? 'all';
      await onGradeAllMatrix(mode === 'ungraded');
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-grade-failed-matrix]').forEach((button) => {
    button.addEventListener('click', async () => {
      await onGradeFailedMatrix();
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-matrix-density-toggle]').forEach((button) => {
    button.addEventListener('click', () => {
      onToggleMatrixDensity();
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-grade-matrix]').forEach((button) => {
    button.addEventListener('click', async () => {
      const { assessmentId, studentId } = getCellMeta(button);
      if (!assessmentId || !studentId) return;
      await onGradeMatrix(assessmentId, studentId);
    });
  });
}
