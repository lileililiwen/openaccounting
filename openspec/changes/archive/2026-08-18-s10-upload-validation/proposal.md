# Upload Size Limits and MIME Validation

## Why

Document upload (`src/handlers/documents.rs`) accepts multipart without
a body cap. A logged-in user can upload 10 GB of garbage and fill the disk.
MIME is taken from the upload header without sniffing; a `text/html` file
with `.pdf` extension becomes a stored XSS vector if ever rendered (the
template would escape, but other clients may not).

## What Changes

- Default 25 MB cap per upload; configurable via `UPLOAD_MAX_BYTES` env.
- Axum `DefaultBodyLimit` middleware enforces the cap before the handler
  reads the body.
- Sniff the first 4 KB with the `infer` crate to validate MIME; reject if
  the sniff disagrees with the upload's Content-Type (with a small list of
  trusted exceptions, e.g. CSV).
- Store the sniffed MIME; the upload header is logged but not trusted.

## Capabilities

### New Capabilities

- `upload-validation`: Upload size limit + content-sniff MIME.

## Impact

**New dependencies:**
- `infer = "0.16"` for content sniffing.
- `tower-http::limit::DefaultBodyLimit` (already available via tower-http).

**Modified files:**
- `src/lib.rs` — install `DefaultBodyLimit`.
- `src/handlers/documents.rs` — sniff + reject.
- `src/handlers/bank_feeds.rs` — same for statement CSVs.
- `src/config.rs` — `upload_max_bytes`.
- `README.md` — document the env var.
