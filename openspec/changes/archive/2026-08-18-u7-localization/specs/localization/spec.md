# localization Specification (delta)

## ADDED Requirements

### Requirement: Locale Selection

MUST pick the locale in this order: explicit user setting (`/account/locale`), `Accept-Language` header, default `en`.

#### Scenario: Accept-Language

- **WHEN** the header requests `zh-CN`
- **THEN** Simplified Chinese strings are returned.

### Requirement: User Override

MUST allow the user to pick a locale in `/account/locale`; the choice is stored and overrides the header.

#### Scenario: Override

- **WHEN** the user picks `es`
- **THEN** Spanish strings are returned regardless of `Accept-Language`.

### Requirement: Plural Forms

MUST use a plural-form helper for plurals (`one transaction`, `N transactions`).

#### Scenario: Plural

- **WHEN** the count is 5
- **THEN** the rendered text is the plural form.

### Requirement: Date/Number Formatting

MUST format dates and numbers per locale.

#### Scenario: Date format

- **WHEN** locale is `de`
- **THEN** dates render as `31.12.2025`.

### Requirement: Missing Key Fallback

MUST fall back to the English string when a key is missing in the active locale; the missing key MUST be logged.

#### Scenario: Missing key

- **WHEN** a key is missing in `es`
- **THEN** the English value is rendered; a warning is logged.

### Requirement: Currency Formatting

MUST format currencies per locale (separator, symbol position, decimal places).

#### Scenario: EUR in de

- **WHEN** the locale is `de`
- **THEN** the amount renders as `1.234,56 €`.
