/**
 * EduTrack Micro-Interactions & Animation Engine
 * Self-initializing module — adds ripple effects, scroll reveals,
 * card stagger entrances, and mouse-tracking glow via event delegation
 * so everything survives full DOM re-renders.
 */

// ── Ripple Effect (delegated) ──────────────────────────────────────
const RIPPLE_SELECTORS = [
  '.primary-button', '.ghost-button', '.danger-button',
  '.segmented button', '.nav-item', '.class-pill',
  '.attendance-cell', '.filter-chip', '.lesson-status-chip',
];

function spawnRipple(el: HTMLElement, e: PointerEvent) {
  const rect = el.getBoundingClientRect();
  const size = Math.max(rect.width, rect.height) * 2;
  const x = e.clientX - rect.left - size / 2;
  const y = e.clientY - rect.top - size / 2;

  const ripple = document.createElement('span');
  ripple.className = 'ripple-wave';
  ripple.style.cssText = `width:${size}px;height:${size}px;left:${x}px;top:${y}px`;

  // Ensure parent can contain the ripple
  const pos = getComputedStyle(el).position;
  if (pos === 'static') el.style.position = 'relative';
  const prev = el.style.overflow;
  el.style.overflow = 'hidden';

  el.appendChild(ripple);
  ripple.addEventListener('animationend', () => {
    ripple.remove();
    el.style.overflow = prev;
  }, { once: true });
}

document.addEventListener('pointerdown', (e) => {
  const target = e.target as HTMLElement;
  for (const sel of RIPPLE_SELECTORS) {
    const match = target.closest<HTMLElement>(sel);
    if (match) {
      spawnRipple(match, e);
      break;
    }
  }
}, { passive: true });

// ── Scroll-Reveal via IntersectionObserver ──────────────────────────
const REVEAL_SELECTORS = [
  '.metric-card', '.agent-card', '.student-card', '.school-row',
  '.syllabus-unit-card', '.submission-card', '.report-student-card',
  '.lesson-preview-row', '.schedule-card', '.calendar-section',
  '.test-row', '.panel', '.hero-panel', '.context-chip', '.pulse-item',
];

const revealObserver = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        (entry.target as HTMLElement).classList.add('revealed');
        revealObserver.unobserve(entry.target);
      }
    }
  },
  { threshold: 0.08, rootMargin: '0px 0px -30px 0px' },
);

function observeRevealTargets() {
  for (const sel of REVEAL_SELECTORS) {
    document.querySelectorAll<HTMLElement>(sel).forEach((el) => {
      if (!el.classList.contains('reveal-ready') && !el.classList.contains('revealed')) {
        el.classList.add('reveal-ready');
        revealObserver.observe(el);
      }
    });
  }
  assignStaggerDelays();
}

// ── Stagger Delays for Grid Children ────────────────────────────────
const STAGGER_GRIDS = [
  '.metric-grid', '.agent-grid', '.student-grid', '.school-directory',
  '.syllabus-units-grid', '.tests-submissions-grid', '.reports-grid',
  '.pulse-rail', '.schedule-board',
];

function assignStaggerDelays() {
  for (const gridSel of STAGGER_GRIDS) {
    document.querySelectorAll<HTMLElement>(gridSel).forEach((grid) => {
      if (grid.dataset.staggered) return;
      grid.dataset.staggered = '1';
      const children = grid.children;
      for (let i = 0; i < children.length; i++) {
        const child = children[i] as HTMLElement;
        child.style.setProperty('--stagger', `${i * 60}ms`);
      }
    });
  }
}

// ── Mouse-tracking Glow on Cards ────────────────────────────────────
const GLOW_SELECTORS = [
  '.metric-card', '.agent-card', '.student-card', '.school-row',
  '.syllabus-unit-card',
];

document.addEventListener('mousemove', (e) => {
  for (const sel of GLOW_SELECTORS) {
    const cards = document.querySelectorAll<HTMLElement>(sel);
    cards.forEach((card) => {
      const rect = card.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      card.style.setProperty('--glow-x', `${x}px`);
      card.style.setProperty('--glow-y', `${y}px`);
    });
  }
}, { passive: true });

// ── Auto-reinitialize on DOM changes ────────────────────────────────
const domObserver = new MutationObserver(() => {
  observeRevealTargets();
});

function init() {
  domObserver.observe(document.body, { childList: true, subtree: true });
  observeRevealTargets();
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init, { once: true });
} else {
  init();
}

export {};
