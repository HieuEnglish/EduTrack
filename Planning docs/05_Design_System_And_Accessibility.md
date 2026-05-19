# 05 — Design system and accessibility

**Project:** EduTrack  
**Version:** 1.1

---

## Design philosophy

The app should now communicate both:
- daily teaching activity
- yearly planning confidence

The UI must make hierarchy and plan status obvious without feeling heavy.

---

## Colour palette

### Panel colours (primary semantic assignments)
- Today / attendance: rose
- Session / lesson log: mint
- AI insights: lavender
- Progress / coverage: amber
- Calendar / planning: sky blue
- Syllabus / curriculum: teal

### Status colours
- success: green
- warning: amber
- error: red
- neutral: slate

### Planning-specific colour rules
- teachable day: soft blue tint
- holiday/closure: muted red tint with strike or blocked pattern
- buffer day: pale purple tint
- completed planned lesson: green accent
- drift / behind plan: amber/red accent

---

## Typography
- same base typography as existing app
- hierarchy chips should use medium weight
- planning warnings use short bold labels followed by plain-language detail

---

## Component patterns

### Context chip row
Displays:
- School
- Level
- Class

Each chip is clickable and keyboard accessible.

### Weekday toggle group
- seven pill buttons
- selected state clearly filled
- accessible labels such as "Monday selected"

### Syllabus upload card
- dashed drop zone
- file metadata row
- parse/extraction status badge
- retry action when failed

### Calendar cell
- minimum touch target 44×44
- date number top-left
- optional small markers for holiday / closure / lesson

### Plan lesson row
- date
- lesson title
- curriculum unit
- status
- warning icon if drifted or moved

### Character level pips
Retained from previous design.

---

## Accessibility

- all interactive elements keyboard reachable
- calendar grid must have screen-reader labels including date and status
- colour is never the only indicator for holiday/buffer/completed states
- progress and generation stages announced with `aria-live`
- upload and background job states must be perceivable without motion dependence

---

## Responsive behaviour

- planning screens stack cleanly on narrower desktop widths
- sticky summary bar should remain readable
- large calendar views should support horizontal compression without losing labels
