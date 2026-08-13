# Fix critical bugs and quality — Design

## Context

The audit found 8 high-priority issues and 1 medium-priority
issue across the codebase. This change fixes all of them in a
single pass.

## H1: Transaction Form Posting Indices

The transaction form template uses a `{% for line in form.lines %}`
loop but hardcodes all field names to `lines[0][field]`. The fix:

```html
{% for line in form.lines %}
{% set idx = loop.index0 %}
<select name="lines[{{ idx }}][account_id]" ...>
<select name="lines[{{ idx }}][direction]" ...>
<input name="lines[{{ idx }}][amount]" ...>
<input name="lines[{{ idx }}][memo]" ...>
{% endfor %}
```

The JS clone handler in `static/app.js` must also rewrite indices
when adding new rows.

## H2: ensure_owner Existence Leak

Change `AppError::Forbidden` to `AppError::NotFound` in
`src/handlers/ledgers.rs:179`.

## H3-H4: Header Injection

Add a helper:

```rust
fn sanitize_header_value(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '"')
        .collect()
}
```

Apply in:
- `src/handlers/documents.rs:201` (download)
- `src/handlers/reports.rs:264` (CSV export)

## H5: Remove unwrap()

Replace `Body::empty().unwrap()` with:

```rust
axum::body::Body::empty()
```

or use `Redirect::to()` which returns a proper response.

## H6: Remove Dead Code

Delete:
- `src/handlers/ledgers.rs:183` `now()` function
- `src/handlers/account.rs:83` `_user_marker()` function
- `src/templates/account.rs:96` `_UUID_MARKER` constant

For `src/reports/general_ledger.rs:129` `type_for_total()` — check
if it's actually dead before deleting.

## H7: forbid(unsafe_code)

Add to `src/main.rs`:

```rust
#![forbid(unsafe_code)]
```

## H8: Date Range Validation

Add to each report handler (or a shared helper):

```rust
if from > to {
    return Err(AppError::Validation("from must be <= to".into()));
}
```

## M3: General Ledger SQL No-Op

Remove the no-op from the SQL:

```sql
-- Before:
+ CASE WHEN a.type IN ('ASSET','EXPENSE') THEN 0 ELSE 0 END
-- After: (remove the line entirely)
```

The Rust code at lines 118-123 already handles the sign flip
correctly.

## Tests

### Unit

- `sanitize_header_value` strips control chars and `"`.
- Report handlers return 400 when `from > to`.
- `ensure_owner` returns NotFound for non-owned ledgers.

### Manual

- Create a 2-posting transaction via UI → verify both postings
  are saved with correct indices.
