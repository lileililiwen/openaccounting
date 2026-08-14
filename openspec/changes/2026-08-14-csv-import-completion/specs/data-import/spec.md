# data-import Specification (delta)

## MODIFIED Requirements

### Requirement: Generic CSV Importer

(Replaces the existing generic-CSV behaviour, which was a stub.)

`POST /ledgers/{id}/import/confirm` SHALL accept a form whose
hidden inputs echo the parsed preview (filename, column
mapping, all rows, the user-chosen `default_account_id`, and
`skip_duplicates: bool`).

The handler SHALL, for every non-duplicate row in the preview:

1. Resolve the row's `account` field (column 5) to a ledger
   account by case-insensitive exact name match; if absent,
   use `default_account_id`.
2. Parse `txn_date` as ISO `YYYY-MM-DD`; reject the row with a
   `BadRow` error if the format is wrong.
3. Parse `amount` (debit OR credit) as `Decimal` ≥ 0.01; reject
   rows where neither debit nor credit is set, where both are
   set, or where either fails to parse.
4. Insert one `transactions` row and two `postings` (DR
   resolved account; CR the ledger's default cash account
   identified by `type='ASSET' AND subtype='cash'`). The
   transaction description is the row's `description` field;
   the payee is `payee`; the reference is `reference`.
5. Increment a per-batch counter.

The whole batch SHALL run inside a single Postgres transaction.
If any single row fails validation or the
`check_posting_balance` trigger fires, the entire batch is
rolled back and the response is `422 Unprocessable Entity` with
a body listing each error as `Row <n>: <message>`.

If all rows succeed, the response is `303 See Other` to
`/ledgers/{id}/transactions?from=<earliest txn_date>`.

#### Scenario: Successful commit creates N transactions

- **WHEN** the user commits a preview of 12 valid rows with
  `default_account_id=<id of "Other Expense">` and
  `skip_duplicates=true`
- **THEN** 12 new `transactions` rows are created (or fewer if
  some are duplicates), each with two postings, and the
  response is `303 See Other` to the transactions list.

#### Scenario: Bad row rolls back the batch

- **WHEN** the preview contains 1 row whose `txn_date` is
  `2026-13-99` and 9 valid rows
- **THEN** the response is `422` with body
  `Row 1: invalid date "2026-13-99". 0 transactions committed.`,
  and zero rows are added to `transactions` or `postings`.

#### Scenario: Both debit and credit set is rejected

- **WHEN** a row has `debit=100.00` AND `credit=100.00`
- **THEN** the response is `422` with body
  `Row <n>: exactly one of debit or credit must be set.`.

### Requirement: Preview Cap Removed

The preview handler `POST /ledgers/{id}/import` SHALL parse up
to 10,000 data rows (raised from the current 10-row cap) and
SHALL stream the parsed result into the Askama template without
allocating intermediate `String`s per row.

If the file exceeds 10,000 rows, the preview page SHALL display
a warning banner "Showing the first 10,000 rows. The full file
will be processed on commit." — the commit handler SHALL
re-parse the entire uploaded file (which is stored in a temp
file written by the multipart extractor) and process all rows.

#### Scenario: 500-row file

- **WHEN** the user uploads a 500-row CSV
- **THEN** the preview shows all 500 rows and the commit
  creates 500 transactions (or fewer with duplicates skipped).

### Requirement: Duplicate Skip

When `skip_duplicates=true` (the default), the commit handler
SHALL skip any row whose
`(txn_date, amount_cents, normalized_payee)` fingerprint
matches a transaction already in the ledger. A row whose
fingerprint matches another row **within the same upload** is
also skipped.

The preview page SHALL flag duplicates with a yellow background
so the user can see what will be skipped before committing.

The fingerprint is computed by the `import::dedup` helper
introduced by the WeChat/Alipay change.
