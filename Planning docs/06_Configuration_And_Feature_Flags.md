# 06 — Configuration and feature flags

**Project:** EduTrack  
**Version:** 1.1

---

## Purpose

This document defines configuration and flags for the expanded planning model.

---

## Configuration layers

### 1) Build-time configuration
- app version
- release channel
- bundled parser/LLM defaults

### 2) Device runtime configuration
- local model endpoint/path
- parser worker limits
- diagnostics level

### 3) School-scoped configuration
- default timezone
- default region
- default report word count

### 4) Level-scoped configuration
- academic year defaults
- active syllabus defaults
- default calendar selection
- planning buffer strategy

### 5) Class-scoped configuration
- meeting-day rules
- agent settings
- dashboard defaults
- class-specific plan override

---

## Configuration storage plan

### In database
- school defaults
- level defaults
- class settings
- feature flags

### In local config file
- parser fallback settings
- model runtime path
- debug options

### In OS keychain / secure storage
- credentials for optional future sync only

---

## Core configuration keys

### App settings
| Key | Type | Default |
|---|---|---|
| `app.locale` | string | `en` |
| `app.timezone` | string | device timezone |
| `app.sidebar.compact` | boolean | `false` |

### Model settings
| Key | Type | Default |
|---|---|---|
| `model.provider` | string | `local` |
| `model.path_or_endpoint` | string | `` |
| `model.max_parallel_jobs` | integer | `1` |

### Parser settings
| Key | Type | Default |
|---|---|---|
| `parser.ocr_enabled` | boolean | `false` |
| `parser.max_file_mb` | integer | `25` |
| `parser.fallback_lesson_estimate` | integer | `1` |

### School settings
| Key | Type | Default |
|---|---|---|
| `school.default_word_count` | integer | `200` |
| `school.default_region` | string | `` |
| `school.default_timezone` | string | device timezone |

### Level settings
| Key | Type | Default |
|---|---|---|
| `level.planning.buffer_percent` | integer | `10` |
| `level.planning.pacing_mode` | string | `balanced` |
| `level.calendar.auto_fill_holidays` | boolean | `true` |
| `level.syllabus.require_review_before_publish` | boolean | `true` |

### Class settings
| Key | Type | Default |
|---|---|---|
| `class.max_students` | integer | `50` |
| `class.character_level_max` | integer | `10` |
| `class.dashboard.default_date_mode` | string | `today` |
| `class.plan.inherit_from_level` | boolean | `true` |

---

## Feature flag goals

- allow phased rollout of planning features
- allow safe fallback to the old operational core if a planning module is unstable
- enable beta planning tools without affecting all classes

---

## Flag design

| Flag | Default | Scope | Purpose |
|---|---|---|---|
| `feature.levels.enabled` | true | global | Enable level/grade hierarchy |
| `feature.calendar.enabled` | true | global/level | Enable built-in academic calendar |
| `feature.syllabus_upload.enabled` | true | global/level | Enable syllabus upload UI |
| `feature.year_plan_generation.enabled` | true | global/level/class | Enable AI-assisted planning |
| `feature.plan_actual_linking.enabled` | true | class | Link sessions to planned lessons |
| `feature.reports.enabled` | true | global/class | Enable report generation UI |
| `feature.agents.enabled` | true | class | Enable agent processing |
| `feature.character_progression.enabled` | true | class | Enable avatar/level system |

---

## Resolution order

When reading config for a class:
1. build-time default
2. device runtime config
3. school override
4. level override
5. class override

---

## Failure handling rules

- invalid `level.planning.buffer_percent` falls back to 10
- missing region disables auto-holiday fill but not calendar creation
- missing meeting-day rules blocks year-plan generation for that class
- invalid model config disables AI planning/report features only
- parser failures should not block attendance or grading flows

---

## Admin-facing settings screens

- school settings
- level planning settings
- class settings
- runtime/model settings
- diagnostics and feature flags
