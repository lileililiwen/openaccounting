# Dark Mode Toggle

## Why

No dark mode. Every modern accounting app ships one. Tailwind makes
this a small change.

## What Changes

- Add the `dark:` variant in templates where it makes sense.
- A toggle in the nav switches the `<html>` `class`.
- Default follows `prefers-color-scheme`.
- Stored per user (`/account/theme`).

## Capabilities

### New Capabilities

- `dark-mode`: Dark mode toggle.

## Impact

**Modified files:**
- `templates/base.html` — `dark:` classes, toggle script.
- `static/css/app.css` — dark variants.
- `migrations/0044_add_user_theme.sql` (theme column on users).
