# upload-validation Specification

## Purpose
TBD - created by archiving change s10-upload-validation. Update Purpose after archive.
## Requirements
### Requirement: Body Size Limit

MUST reject any request whose body exceeds `UPLOAD_MAX_BYTES` (default 25 MB) with HTTP 413.

#### Scenario: 30 MB upload

- **WHEN** the user uploads 30 MB
- **THEN** 413.

#### Scenario: 25 MB exactly

- **WHEN** the user uploads 25 MB exactly
- **THEN** the upload succeeds.

### Requirement: MIME Sniff

MUST sniff the first 4 KB of the upload to determine the real MIME type; MUST reject (400) when the sniff result disagrees with the declared Content-Type, with the following exceptions: CSV (`text/csv`), which is accepted on declaration because sniff can be ambiguous for small files.

#### Scenario: PDF declared, sniffed PDF

- **WHEN** the user uploads a real PDF
- **THEN** accepted.

#### Scenario: HTML renamed to PDF

- **WHEN** the user uploads an HTML file with `.pdf` extension
- **THEN** rejected with `Content does not match declared type`.

#### Scenario: CSV declared, sniffed text

- **WHEN** the user uploads a CSV
- **THEN** accepted.

#### Scenario: Empty file

- **WHEN** the user uploads 0 bytes
- **THEN** rejected with `Empty upload`.

### Requirement: Trusted Extension Map

MUST maintain a small static map of (extension, sniffed-type) tuples; the sniffed type wins over the header for known office / archive formats where browsers mislabel.

#### Scenario: XLSX with text/plain

- **WHEN** the user uploads a real XLSX labeled as text/plain
- **THEN** accepted; stored MIME is `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`.

### Requirement: Quota Notification

MUST emit a structured warning log when a single user exceeds 1 GB of total uploads in a 24 h period.

#### Scenario: Quota log

- **WHEN** the user uploads 200 MB five times
- **THEN** a `WARN` log entry is emitted with the user_id and the running total.

