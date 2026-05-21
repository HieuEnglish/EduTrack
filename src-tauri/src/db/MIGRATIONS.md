# EduTrack SQLite Migrations

EduTrack uses startup-applied, versioned migrations recorded in `schema_migrations`.

## How It Works

1. New installs:
- `schema.sql` is applied to create the latest schema snapshot.
2. Existing installs:
- The app checks `schema_migrations`.
- Unapplied migrations are executed in order.
- Each successful migration is recorded with version + timestamp.

Migration execution is implemented in:

- `src-tauri/src/services/hierarchy.rs` via `apply_schema_migrations(...)`
- Migration registry constant: `MIGRATIONS`

## Current Migration Versions

- `001_add_student_age_gender`
- `002_add_period_minutes`
- `003_planning_foundation`
- `004_detailed_plan_attachment`
- `005_curriculum_units_month_label`

## Adding A New Migration

1. Add a new migration function in `hierarchy.rs`.
2. Register it in the `MIGRATIONS` array with a new version id.
3. Keep migration steps idempotent where possible (`IF NOT EXISTS`, `add_column_if_missing`, etc.).
4. Update `schema.sql` so fresh installs still get the newest full schema.
5. Test both cases:
- Fresh database creation.
- Upgrade from an older database with existing user data.

## Why This Matters

Without deterministic migrations, updates can break local teacher databases.  
This strategy ensures schema evolution is explicit, ordered, and backward-compatible.
