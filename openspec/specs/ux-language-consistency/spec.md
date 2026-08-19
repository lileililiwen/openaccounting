# ux-language-consistency Specification

## Purpose
The web UI is English-only, but browser-native controls (HTML5 validation tooltips, the file picker) and the default 404 fallback render in the OS locale (here: Simplified Chinese). A user of an English app must never see browser-generated UI in a different language; the app MUST present a single consistent language and SHOULD ship its own error and form-feedback surfaces instead of delegating to the browser.

Found by a novice-user form submission test on 2026-08-19 (Chrome, zh-CN system locale). Evidence: `docs/ux-novice-form-test/*.png`.

## Requirements

### Requirement: Custom Form Validation Messages

MUST replace browser-native `required`/`minlength`/`type` validation tooltips with app-owned messages in the UI language; the app SHALL listen for `invalid` events (or use `setCustomValidity`) and render its own styled message.

#### Scenario: Password shorter than minimum

- **WHEN** a user submits the register form with a 5-character password (minlength 12)
- **THEN** the app shows an English, app-styled message such as "At least 12 characters" in place of the browser's localized tooltip (observed: "请将该文本增加为 12 个字符或更多", see `04_register_short_pw.png`).

#### Scenario: Required field left empty

- **WHEN** a user submits the transaction form with the description empty
- **THEN** the app shows its own message instead of the browser's localized "请填写此字段" (see `22_save_empty.png`).

#### Scenario: Required select not chosen

- **WHEN** a user submits a posting line without an account
- **THEN** the app shows its own message instead of the browser's localized "请在列表中选择一项" (see `23_save_unbalanced.png`).

### Requirement: Localized File Picker

MUST render the document-upload control with app-language labels instead of the native browser chrome; SHALL wrap the native input in a styled label whose text and "no file chosen" state are in the UI language.

#### Scenario: Upload area in an English app

- **WHEN** a user views the document upload area
- **THEN** the picker reads "Choose file" / "No file chosen" rather than the localized "选择文件" / "未选择任何文件" (see `24_save_both_debit.png`).

### Requirement: Branded 404 Page

MUST respond to unknown paths with the app's own styled error page in the UI language, including a link back to the application; MUST NOT fall through to the browser's default error page.

#### Scenario: Unknown path

- **WHEN** a user visits `/form` (a non-existent route)
- **THEN** the app renders its branded 404 page with a "back to your ledgers" link instead of the Chrome default "找不到 127.0.0.1 的网页 / HTTP ERROR 404" (see `00_form_url.png`).

### Requirement: Declared Document Language

SHOULD declare the rendered UI language on the document (`<html lang="en">`) so browser chrome, spellcheck, and assistive tech align with the visible content.

#### Scenario: English UI

- **WHEN** the app renders an English page
- **THEN** the document root declares `lang="en"`.

## Out of Scope
- Translating the application itself into other languages (see the existing `localization` spec).
- Reworking the native file input behaviour beyond its visible labels.
