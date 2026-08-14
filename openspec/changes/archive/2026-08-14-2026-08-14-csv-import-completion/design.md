# CSV Import Completion — Design

## Confirm handler shape

```rust
// src/handlers/import.rs
pub async fn confirm(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    mut multipart: Multipart,    // accept either Form or Multipart
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    // 1. Re-parse the uploaded file from a temp file written
    //    by axum's multipart extractor. The temp path is passed
    //    back via a hidden form field; if missing, the user
    //    re-uploaded the form and we re-process from the new
    //    upload.
    let (rows, mapping, default_account_id, skip_duplicates) =
        read_confirm_form(&mut multipart).await?;

    // 2. Validate ledger accounts up front so we fail fast.
    let cash_account = lookup_cash_account(&state.pool, ledger_id).await?;
    let expense_account = lookup_account(&state.pool, ledger_id,
        default_account_id).await?
        .ok_or(AppError::Validation("default_account_id invalid".into()))?;

    // 3. Open a transaction and process all rows.
    let mut tx = state.pool.begin().await?;
    let mut committed: usize = 0;
    let mut errors: Vec<String> = vec![];

    for (i, row) in rows.iter().enumerate() {
        if skip_duplicates && row.is_duplicate { continue; }
        match insert_one(&mut tx, ledger_id, row, expense_account,
                         cash_account).await {
            Ok(()) => committed += 1,
            Err(e) => errors.push(format!("Row {}: {}", i + 1, e)),
        }
    }

    if !errors.is_empty() {
        tx.rollback().await?;
        return Err(AppError::Validation(format!(
            "{}; 0 transactions committed.", errors.join("; ")
        )));
    }
    tx.commit().await?;

    let _ = audit::log(...).await;
    Ok(Redirect::to(&format!("/ledgers/{}/transactions", ledger_id))
       .into_response())
}
```

## `insert_one`

```rust
async fn insert_one(
    tx: &mut PgConnection,
    ledger_id: Uuid,
    row: &ParsedRow,
    expense_account: Uuid,
    cash_account: Uuid,
) -> Result<(), AppError> {
    let date = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
        .map_err(|e| AppError::Validation(format!("invalid date '{}': {}", row.date, e)))?;

    let (amount, direction) = match (row.debit.as_str(), row.credit.as_str()) {
        (d, "") if !d.is_empty() => (d.parse::<Decimal>().map_err(...)?
                                      .round_dp(2), Direction::Debit),
        ("", c) if !c.is_empty() => (c.parse::<Decimal>().map_err(...)?
                                      .round_dp(2), Direction::Credit),
        _ => return Err(AppError::Validation(
            "exactly one of debit or credit must be set".into())),
    };
    if amount <= Decimal::ZERO {
        return Err(AppError::Validation("amount must be > 0".into()));
    }

    let txn_id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, payee, reference)
         VALUES ($1,$2,$3,$4,$5) RETURNING id"
    )
    .bind(ledger_id).bind(date).bind(&row.description)
    .bind(&row.payee).bind(&row.reference)
    .fetch_one(&mut **tx).await?;

    let (dr_acct, cr_acct) = match direction {
        Direction::Debit  => (expense_account, cash_account),
        Direction::Credit => (cash_account, expense_account),
    };
    sqlx::query(
        "INSERT INTO postings (transaction_id, account_id, direction, amount, currency)
         VALUES ($1, $2, 'DEBIT',  $3, $4),
                ($1, $5, 'CREDIT', $3, $4)"
    )
    .bind(txn_id).bind(dr_acct).bind(amount).bind("CNY")
    .bind(cr_acct)
    .execute(&mut **tx).await?;
    // The check_posting_balance trigger fires here and enforces
    // the invariant.

    Ok(())
}
```

## Preview cap raise

Change the loop in `upload()` from `if i >= 10 { break; }` to
`if i >= 10_000 { break; }` and add the "Showing the first
10,000 rows" banner in the template.

## Account resolution

`row.account` is resolved by case-insensitive exact match in
the same ledger. If absent or unresolved, the handler falls
back to the form's `default_account_id`. The preview page MUST
flag unresolved accounts per row.

## Tests

### Integration (`tests/integration/csv_import.rs`)

- `http_csv_import_creates_n_transactions` — 12 rows → 12 rows
  in DB.
- `http_csv_import_rolls_back_on_bad_date` — 1 bad date + 9 good
  → 0 committed, 422.
- `http_csv_import_rejects_both_debit_and_credit` — 422.
- `http_csv_import_skip_duplicates` — duplicate row excluded
  from the count.
- `http_csv_import_preview_shows_500_rows` — assert preview
  HTML contains all 500 rows.

### Property

- `prop_csv_import_round_trips_balanced_books` — for any
  random batch of N well-formed rows, the ledger's trial
  balance after import equals the trial balance before import
  plus the sum of all imported debits (which equals the sum of
  credits by construction).

## References

- Existing `src/handlers/import.rs` (the stub we're replacing)
- `migrations/0001_init.sql` `check_posting_balance` trigger
- `openspec/changes/2026-08-14-wechat-alipay-import` for the
  shared `ParsedRow` and `dedup` modules.
