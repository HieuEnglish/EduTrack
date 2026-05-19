# EduTrack Design System

EduTrack should feel like a calm, high-trust teacher operations workspace: useful, alive, and focused on daily classroom decisions. It is not a marketing site and should avoid decorative excess.

## Visual Direction

- Tone: crisp, professional, warm, and operational.
- Layout: dashboard-first, dense enough for repeated teacher use, with clear hierarchy and fast scanning.
- Surfaces: white or near-white panels on a lightly textured academic workspace background.
- Shape language: compact 4px to 8px radii; avoid oversized rounded cards.
- Motion: staged entry and state feedback only. Motion should clarify hierarchy or user action.

## Tokens

Use CSS variables in `frontend/src/style.css` as the source of truth.

- Text: `--ink`, `--ink-strong`, `--muted`, `--muted-strong`
- Surfaces: `--paper`, `--surface`, `--surface-solid`, `--surface-warm`
- Lines: `--line`, `--line-strong`
- Accents: `--teal`, `--blue`, `--amber`, `--violet`, `--rose`, `--green`
- Radius: `--radius-xs`, `--radius-sm`, `--radius-md`, `--radius-full`
- Motion: `--ease`, `--ease-emphatic`, `--duration-fast`, `--duration`, `--duration-slow`

## Interaction Rules

- Primary actions use teal-to-blue treatment.
- Secondary actions use white surfaces with borders.
- Segmented controls are used for mode switching.
- Empty states should be honest and actionable, not filled with fake demo data.
- Cards can lift slightly on hover, but they should not jump, wobble, or distract.

## Content Rules

- Prefer teacher-centered labels: plan, teach, review, classroom, roster, attendance.
- Avoid synthetic metrics. Show `-`, `No data`, or an empty state when records do not exist.
- Keep headings short and concrete.
