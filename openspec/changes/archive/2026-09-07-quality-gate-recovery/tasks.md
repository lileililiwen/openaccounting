# Tasks

## 1. Testing

- [x] 1.1 Add unit coverage for single-amount header detection, debit/credit precedence, and unmapped optional fields.
- [x] 1.2 Add a setup test or command-level check proving `.env.test` is used only when `DATABASE_URL` is absent.
- [x] 1.3 Capture the exact local lint and test commands in contributor documentation.

## 2. Implementation

- [x] 2.1 Fix `auto_detect` to map supported amount headers while preserving `-1` sentinels and debit/credit precedence.
- [x] 2.2 Resolve all current Clippy errors and warnings without broad lint suppression or test deletion.
- [x] 2.3 Make test configuration load `.env.test` explicitly with exported environment precedence and actionable failure messages.
- [x] 2.4 Align CI commands and step boundaries with the documented local verification sequence.

## 3. Verification

- [x] 3.1 Run `cargo fmt -- --check`.
- [x] 3.2 Run `cargo clippy --features test-support --all-targets -- -D warnings`.
- [x] 3.3 Run pure library tests and the fresh-database integration suite.
- [x] 3.4 Record the passing commands and any unavailable infrastructure in the change archive.
