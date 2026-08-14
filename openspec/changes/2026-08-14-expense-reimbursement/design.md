# Expense Reimbursement — Design

## Schema (migration 0019_add_reimbursement.sql)

```sql
CREATE TABLE reimbursement_claims (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    author_id       UUID NOT NULL REFERENCES users(id),
    title           TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    description     TEXT NOT NULL DEFAULT '',
    currency        CHAR(3) NOT NULL,
    status          TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft','submitted','approved','rejected','paid')),
    approver_id     UUID REFERENCES users(id),
    reject_reason   TEXT,
    paid_at         TIMESTAMPTZ,
    payout_account_id UUID REFERENCES accounts(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reimbursement_claims_ledger_status_idx
    ON reimbursement_claims (ledger_id, status, created_at DESC);

CREATE TABLE reimbursement_lines (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id                UUID NOT NULL REFERENCES reimbursement_claims(id) ON DELETE CASCADE,
    txn_date                DATE NOT NULL,
    category                TEXT NOT NULL
        CHECK (category IN ('travel','meals','lodging','supplies','software','other')),
    payee                   TEXT NOT NULL DEFAULT '',
    description             TEXT NOT NULL DEFAULT '',
    amount                  NUMERIC(20,4) NOT NULL CHECK (amount > 0),
    currency                CHAR(3) NOT NULL,
    tax_amount              NUMERIC(20,4) NOT NULL DEFAULT 0 CHECK (tax_amount >= 0),
    tax_recoverable         BOOLEAN NOT NULL DEFAULT FALSE,
    gl_account_id           UUID NOT NULL REFERENCES accounts(id),
    project_id              UUID,                   -- FK added later
    receipt_document_id     UUID REFERENCES documents(id),
    applied_advance_id      UUID,                   -- FK added later
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reimbursement_lines_claim_idx
    ON reimbursement_lines (claim_id);

CREATE TABLE reimbursement_events (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id        UUID NOT NULL REFERENCES reimbursement_claims(id) ON DELETE CASCADE,
    actor_id        UUID NOT NULL REFERENCES users(id),
    from_status     TEXT NOT NULL,
    to_status       TEXT NOT NULL,
    reason          TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reimbursement_events_claim_idx
    ON reimbursement_events (claim_id, created_at DESC);
```

`0019_add_reimbursement.sql` also adds a `subtype` value to the
account-subtype CHECK constraint defined in migration `0004`:

```sql
ALTER TABLE accounts DROP CONSTRAINT accounts_subtype_check;
ALTER TABLE accounts ADD CONSTRAINT accounts_subtype_check
    CHECK (subtype IS NULL OR subtype IN (
        'cash','bank','accounts_receivable','accounts_payable',
        'credit_card','owners_equity','sales_revenue','other_income',
        'office_supplies','travel_meals','software_saas','marketing',
        'professional_services','rent','utilities','other_expense',
        'employee_payable','employee_advance','input_vat'   -- NEW
    ));
```

The two new EXPENSE subtypes (`employee_payable` is LIABILITY and
`employee_advance` is ASSET — handled in the seed below).

## Default seeded accounts

In `src/domain/ledger.rs::default_chart_of_accounts`, append:

| Type      | Name               | Subtype           |
|-----------|--------------------|-------------------|
| LIABILITY | Employee Payable   | `employee_payable`|
| ASSET     | Employee Advance   | `employee_advance`|

`Input VAT` is **not** seeded automatically; if the user has none,
`tax_recoverable=true` lines are posted entirely to the GL account
(per spec scenario "Single-line approval posts two postings"
fallback path).

## State machine implementation

```rust
// src/handlers/reimbursement.rs
async fn transition(
    State(state): State<AppState>,
    Path((ledger_id, claim_id)): Path<(Uuid, Uuid)>,
    actor: AuthSession<Backend>,
    to_status: ClaimStatus,
    body: TransitionBody,
) -> AppResult<Response> {
    let user = actor.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_role(&state, user.id, ledger_id, Role::Accountant).await?;

    let mut tx = state.pool.begin().await?;
    let claim = sqlx::query_as::<_, Claim>(
        "SELECT * FROM reimbursement_claims
         WHERE id = $1 AND ledger_id = $2 FOR UPDATE"
    )
    .bind(claim_id).bind(ledger_id)
    .fetch_optional(&mut *tx).await?
    .ok_or(AppError::NotFound)?;

    // validate transition table (see ClaimStatus::can_transition_to)
    if !claim.status.can_transition_to(to_status) {
        return Err(AppError::Conflict(format!(
            "Claim is in {} and cannot move to {}.",
            claim.status, to_status
        )));
    }
    // role/extra checks per status (reject requires reason, etc.)
    ensure_transition_valid(&claim, &body)?;

    // GL posting for the relevant transitions
    match to_status {
        ClaimStatus::Approved => post_approval(&mut tx, &claim).await?,
        ClaimStatus::Paid     => post_payment(&mut tx, &claim, body.payout_account_id).await?,
        _ => {}
    }

    // update claim row + insert event row
    sqlx::query("UPDATE reimbursement_claims SET status=$1, ...")
        .bind(to_status)
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO reimbursement_events (...) VALUES (...)")
        .execute(&mut *tx).await?;

    audit::log(...).await;  // best-effort

    tx.commit().await?;
    Ok(Redirect::to(...).into_response())
}
```

## GL posting helper

```rust
async fn post_approval(tx: &mut PgConnection, claim: &Claim)
    -> AppResult<Uuid>
{
    let lines = sqlx::query_as::<_, Line>(
        "SELECT * FROM reimbursement_lines WHERE claim_id = $1"
    ).bind(claim.id).fetch_all(&mut **tx).await?;

    let mut postings: Vec<(Uuid /*acct*/, Direction, Decimal, &str)> = vec![];

    let input_vat_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM accounts
         WHERE ledger_id = $1 AND type='EXPENSE'
           AND subtype='input_vat' LIMIT 1"
    ).bind(claim.ledger_id).fetch_optional(&mut **tx).await?;

    let payable_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM accounts
         WHERE ledger_id = $1 AND type='LIABILITY'
           AND subtype='employee_payable' LIMIT 1"
    ).bind(claim.ledger_id).fetch_optional(&mut **tx).await?
     .ok_or(AppError::NotFound)?;     // must seed first

    let mut total_dr = Decimal::ZERO;
    for l in &lines {
        postings.push((l.gl_account_id, Direction::Debit, l.amount, "Expense"));
        if l.tax_recoverable && input_vat_id.is_some() && l.tax_amount > Decimal::ZERO {
            postings.push((input_vat_id.unwrap(), Direction::Debit, l.tax_amount, "Input VAT"));
        } else if l.tax_amount > Decimal::ZERO {
            postings.push((l.gl_account_id, Direction::Debit, l.tax_amount, "Expense tax"));
        }
        total_dr += l.amount + l.tax_amount;
    }
    // advance netting
    for l in &lines.iter().filter(|l| l.applied_advance_id.is_some()) {
        postings.push((advance_account, Direction::Debit, l.amount, "Advance applied"));
        total_dr += l.amount;
    }
    postings.push((payable_id, Direction::Credit, total_dr, "Reimbursement payable"));

    // create transaction + postings, return txn_id
    let txn_id = create_transaction_with_postings(tx, claim.ledger_id,
        Utc::now(), &format!("Reimbursement {} (#{})", claim.title, claim.short_id()),
        &postings).await?;

    Ok(txn_id)
}
```

`create_transaction_with_postings` is a new helper in
`src/domain/transaction.rs` that wraps the existing
"insert transaction, then bulk-insert postings, let the trigger
fire" sequence.

## Sharing extension

`src/handlers/ledgers.rs` gains a sibling helper:

```rust
pub async fn ensure_role(state: &AppState, user_id: Uuid,
    ledger_id: Uuid, required: Role) -> AppResult<()>
```

`Role` is a new enum in `src/auth/mod.rs`:

```rust
pub enum Role { Owner, Admin, Accountant, Viewer }
```

`Owner` is the ledger creator. The other three are stored in the
`ledger_shares` table (migration `0008`). `ensure_role` returns
`NotFound` if the user has no share, `Forbidden` if their share
role is below `required`, and `Ok(())` otherwise.

## Templates

`templates/reimbursements/show.html` renders:

- Header with title, author, status badge, total.
- Lines table (date, category, payee, amount, tax, GL account, receipt link).
- "Add line" form (HTMX swap-in row).
- State-action form(s) depending on `status` (Submit / Approve / Reject / Pay).
- Event timeline (newest first) showing actor + from→to + reason.

The forms use the existing HTMX + Tailwind patterns. No new JS
dependencies.

## Tests

### Unit (in `src/handlers/reimbursement.rs` `#[cfg(test)]`)

- `claim_status_can_transition_to` covers the full matrix in both
  directions.
- `normalize_payee` is a no-op (re-uses `import::dedup` for the
  rare case a payee from a claim is later imported).
- `format_short_id` produces the same 8-char base32 slug for the
  same claim id across runs.

### Integration (in `tests/integration/reimbursement.rs`)

- `http_create_draft_claim` — POST → 303, claim visible in list.
- `http_add_line_to_claim` — POST → 303, line visible in show.
- `http_submit_claim_requires_line` — submit empty draft → 400.
- `http_approve_creates_postings` — approve a 1-line claim → 1
  txn with 2 postings.
- `http_approve_with_recoverable_tax` — 3 postings, total
  balanced.
- `http_approve_atomic_on_trigger_violation` — corrupt the GL
  insert → claim stays `submitted`, 0 postings.
- `http_pay_clears_payable` — pay approved → 1 new txn with
  `DR Employee Payable / CR Bank`.
- `http_advance_netting` — line with `applied_advance_id` →
  4-posting txn, `Employee Payable` reduced.
- `http_viewer_cannot_approve` — 403.
- `http_filter_by_status` — list with `?status=submitted` →
  only submitted claims.

### Property

- `prop_state_machine_is_acyclic` — random walks through valid
  transitions never return to a visited status.

## References

- Akaunting `expense-management` docs
- Frappe HRMS Expense Claim doctype
- Expensify report-action state machine
- SAP Concur approval routing
- Zoho Expense per-diem / policies
