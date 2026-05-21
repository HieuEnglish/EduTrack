import type { StudentSubmission, TestTemplate } from '../types';

type RenderTestsPageParams = {
  activeClassName: string | null;
  tests: TestTemplate[];
  currentTestId: string;
  testSubmissions: StudentSubmission[];
  icon: (name: string) => string;
  escapeHtml: (value: string) => string;
  emptyState: (title: string, body: string) => string;
};

type CreateTestInput = {
  title: string;
  instructions: string;
  rubric: string;
  maxScore: number;
};

type SaveScoreInput = {
  studentId: string;
  scoreValue: number | undefined;
  comment: string | null;
};

type BindTestsPageEventsParams = {
  onCreateTest: (input: CreateTestInput) => Promise<void>;
  onGenerateFromSyllabus: () => Promise<void>;
  onSelectTest: (testId: string) => Promise<void>;
  onAutoScore: () => Promise<void>;
  onExportResults: () => void;
  onSaveScore: (input: SaveScoreInput) => Promise<void>;
  onUploadStudentFile: (studentId: string, file: File) => Promise<void>;
  onViewFile: () => void;
};

export function renderTestsPage(params: RenderTestsPageParams): string {
  const {
    activeClassName,
    tests,
    currentTestId,
    testSubmissions,
    icon,
    escapeHtml,
    emptyState,
  } = params;

  const hasClass = Boolean(activeClassName);
  const selectedTest = tests.find((test) => test.id === currentTestId);
  const completedTests = tests.filter((test) => test.status === 'completed').length;
  const pendingTests = Math.max(0, tests.length - completedTests);
  const scoredSubmissions = testSubmissions.filter((entry) => entry.scoreValue != null).length;
  const pendingSubmissions = Math.max(0, testSubmissions.length - scoredSubmissions);

  return `
    <section class="tests-layout">
      <article class="panel tests-create-panel">
        <p class="eyebrow">Test authoring</p><h2>Create a new test</h2><p>Design and review tests here. Student upload + grading happens in Classroom → Marking.</p>
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
      <article class="panel tests-overview-panel">
        <div class="tests-kpis">
          <article><span>Total tests</span><strong>${tests.length}</strong></article>
          <article><span>Completed</span><strong>${completedTests}</strong></article>
          <article><span>Pending</span><strong>${pendingTests}</strong></article>
          <article><span>Scored submissions</span><strong>${scoredSubmissions}</strong></article>
          <article><span>Unscored submissions</span><strong>${pendingSubmissions}</strong></article>
        </div>
        <p class="settings-detail">Workflow split: this page = create/review tests. Classroom → Marking = upload attempts, grade, and class results.</p>
      </article>
      <article class="panel"><div class="panel-head"><p class="eyebrow">Existing tests</p><h2>${activeClassName ? escapeHtml(activeClassName) : 'No class'}</h2></div>${tests.length === 0 ? emptyState('No tests yet', 'Create a test here, then use Classroom → Marking for submissions and grading.') : `<div class="tests-list-tools"><input id="tests-filter" type="search" placeholder="Filter tests..." aria-label="Filter tests" /></div><div class="tests-list">${tests.map((test) => `<div class="test-row ${test.id === currentTestId ? 'active' : ''}" data-select-test="${escapeHtml(test.id)}" data-test-name="${escapeHtml(test.title.toLowerCase())}"><div class="test-row-info"><strong>${escapeHtml(test.title)}</strong><span>${escapeHtml(test.status)} · ${escapeHtml(test.createdAt.slice(0, 10))}</span></div><span class="test-row-status ${test.status}">${test.status === 'completed' ? 'Completed' : 'Pending'}</span></div>`).join('')}</div>`}</article>
      ${selectedTest ? `
        <article class="panel tests-submissions-panel">
          <div class="panel-head"><div><p class="eyebrow">Test review</p><h2>${escapeHtml(selectedTest.title)}</h2></div><div class="tests-actions"><button id="export-test-results" class="ghost-button" type="button">${icon('upload')} Export</button></div></div>
          <p>${escapeHtml(selectedTest.instructions)}</p>
          <div class="tests-max-score"><span>Max score:</span> <strong>${selectedTest.maxScore}</strong></div>
          <p class="input-hint">Use Classroom → Marking to upload student attempts and run grading. This view is read-only for review.</p>
          ${testSubmissions.length === 0 ? emptyState('No submissions loaded', 'Select a test above to review status.') : `<div class="tests-list-tools"><input id="submission-filter" type="search" placeholder="Filter students..." aria-label="Filter submissions" /></div><div class="tests-submissions-grid">${testSubmissions.map((submission) => `<div class="submission-card" data-submission-card="1" data-student-name="${escapeHtml(submission.studentName.toLowerCase())}"><div class="submission-head"><strong>${escapeHtml(submission.studentName)}</strong><span class="submission-status ${submission.status}">${submission.status === 'scored' ? 'Scored' : 'Pending'}</span></div><div class="submission-files">${submission.fileName ? `<span class="submission-file">${escapeHtml(submission.fileName)} <button type="button" class="compact-button ghost-button" data-view-file="${escapeHtml(submission.attachmentId ?? '')}">View</button></span>` : '<span class="submission-file empty">No file submitted</span>'}</div><div class="submission-score-field"><span class="label">Score</span><strong>${submission.scoreValue ?? '-'}</strong></div><div class="submission-comment-field"><span class="label">Feedback</span><p>${escapeHtml(submission.teacherComment ?? 'No feedback yet')}</p></div></div>`).join('')}</div>`}
        </article>` : ''}
    </section>
  `;
}

export function bindTestsPageEvents(params: BindTestsPageEventsParams): void {
  const {
    onCreateTest,
    onGenerateFromSyllabus,
    onSelectTest,
    onAutoScore,
    onExportResults,
    onSaveScore,
    onUploadStudentFile,
    onViewFile,
  } = params;

  document.querySelector<HTMLFormElement>('#test-create-form')?.addEventListener('submit', async (event) => {
    event.preventDefault();
    const title = (document.querySelector<HTMLInputElement>('#test-title')?.value ?? '').trim();
    const instructions = (document.querySelector<HTMLTextAreaElement>('#test-instructions')?.value ?? '').trim();
    const rubric = (document.querySelector<HTMLTextAreaElement>('#test-rubric')?.value ?? '').trim();
    const maxScore = Number(document.querySelector<HTMLInputElement>('#test-max-score')?.value ?? 100);
    await onCreateTest({ title, instructions, rubric, maxScore });
  });

  document.querySelector<HTMLButtonElement>('#test-generate-syllabus')?.addEventListener('click', async () => {
    await onGenerateFromSyllabus();
  });

  document.querySelectorAll<HTMLElement>('[data-select-test]').forEach((element) => {
    element.addEventListener('click', async () => {
      const testId = element.dataset.selectTest ?? '';
      if (!testId) return;
      await onSelectTest(testId);
    });
  });

  document.querySelector<HTMLButtonElement>('#auto-score-test')?.addEventListener('click', async () => {
    await onAutoScore();
  });

  document.querySelector<HTMLButtonElement>('#export-test-results')?.addEventListener('click', () => {
    onExportResults();
  });

  document.querySelectorAll<HTMLButtonElement>('[data-save-score]').forEach((button) => {
    button.addEventListener('click', async () => {
      const studentId = button.dataset.saveScore ?? '';
      if (!studentId) return;
      const scoreInput = document.querySelector<HTMLInputElement>(`[data-score-student="${studentId}"]`);
      const commentInput = document.querySelector<HTMLTextAreaElement>(`[data-comment-student="${studentId}"]`);
      const scoreValue = scoreInput ? parseFloat(scoreInput.value) : undefined;
      const comment = commentInput?.value?.trim() || null;
      await onSaveScore({ studentId, scoreValue, comment });
    });
  });

  document.querySelectorAll<HTMLInputElement>('[data-upload-student]').forEach((input) => {
    input.addEventListener('change', async (event) => {
      const fileInput = event.currentTarget as HTMLInputElement;
      const studentId = fileInput.dataset.uploadStudent ?? '';
      const file = fileInput.files?.[0];
      if (!file || !studentId) return;
      await onUploadStudentFile(studentId, file);
    });
  });

  document.querySelector<HTMLInputElement>('#tests-filter')?.addEventListener('input', (event) => {
    const query = ((event.currentTarget as HTMLInputElement).value || '').trim().toLowerCase();
    document.querySelectorAll<HTMLElement>('[data-select-test]').forEach((row) => {
      const name = row.dataset.testName ?? '';
      row.hidden = query.length > 0 && !name.includes(query);
    });
  });

  document.querySelector<HTMLInputElement>('#submission-filter')?.addEventListener('input', (event) => {
    const query = ((event.currentTarget as HTMLInputElement).value || '').trim().toLowerCase();
    document.querySelectorAll<HTMLElement>('[data-submission-card="1"]').forEach((card) => {
      const name = card.dataset.studentName ?? '';
      card.hidden = query.length > 0 && !name.includes(query);
    });
  });

  document.querySelectorAll<HTMLButtonElement>('[data-view-file]').forEach((button) => {
    button.addEventListener('click', () => {
      onViewFile();
    });
  });
}
