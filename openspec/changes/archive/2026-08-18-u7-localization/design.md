# ## Context

No i18n.

## Goals / Non-Goals

**Goals:**
- 6 languages on day 1.

**Non-Goals:**
- Right-to-left (RTL) — separate.

## Decisions

- JSON files; loaded once at startup into an `Arc<RwLock<HashMap>>`.
- All Askama templates use a custom `t(key)` filter.
- Numbers/dates via `chrono` and `rust_decimal` locale-aware formatters.
