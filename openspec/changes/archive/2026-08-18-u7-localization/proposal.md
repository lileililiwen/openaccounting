# UI Localization (i18n)

## Why

All UI strings are baked-in English. Firefly III ships in 30+
languages via Crowdin; GnuCash in 25+. A non-English freelancer can't use
the product.

## What Changes

- Extract every user-visible string to a `static/locales/<lang>.json`
  file.
- A small runtime picks the locale based on `Accept-Language` (with a
  user preference override).
- Ship English + Simplified Chinese + Spanish + French + German + Japanese
  on day 1.
- All Askama templates render the string via a `t(key)` helper.

## Capabilities

### New Capabilities

- `localization`: Multi-language UI.

## Impact

**New files:**
- `static/locales/en.json`, `zh-CN.json`, `es.json`, `fr.json`,
  `de.json`, `ja.json`.
- `src/i18n/mod.rs`.
- `tests/http/i18n.rs`.
