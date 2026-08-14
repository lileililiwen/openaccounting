# Reimbursement Approval Routing — Design

## Schema

```sql
-- migrations/0022_add_approval_routing.sql
CREATE TABLE reimbursement_approval_policies (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    min_amount      NUMERIC(20,4) NOT NULL CHECK (min_amount >= 0),
    approver_role   TEXT NOT NULL CHECK (approver_role IN ('Admin','Accountant')),
    level           INT NOT NULL CHECK (level >= 1),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX reimbursement_approval_policies_ledger_idx
    ON reimbursement_approval_policies (ledger_id, min_amount, level);

CREATE TABLE reimbursement_approval_steps (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id        UUID NOT NULL REFERENCES reimbursement_claims(id) ON DELETE CASCADE,
    level           INT NOT NULL,
    approver_id     UUID NOT NULL REFERENCES users(id),
    approved_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (claim_id, level, approver_id)
);

CREATE INDEX reimbursement_approval_steps_claim_idx
    ON reimbursement_approval_steps (claim_id, level);
```

## Required-level computation

```rust
// src/domain/approval_routing.rs
pub async fn required_levels(
    pool: &PgPool, ledger_id: Uuid, total: Decimal,
) -> AppResult<Vec<i32>> {
    let rows = sqlx::query_as::<_, (i32,)>(
        "SELECT level FROM reimbursement_approval_policies
         WHERE ledger_id = $1 AND min_amount <= $2
         ORDER BY min_amount, level"
    )
    .bind(ledger_id).bind(total)
    .fetch_all(pool).await?;
    let mut levels: Vec<i32> = rows.into_iter().map(|(l,)| l).collect();
    if levels.is_empty() { levels.push(1); }     // default
    levels.sort(); levels.dedup();
    Ok(levels)
}

pub async fn recorded_levels(
    pool: &PgPool, claim_id: Uuid,
) -> AppResult<Vec<i32>> {
    Ok(sqlx::query_scalar("SELECT DISTINCT level
        FROM reimbursement_approval_steps WHERE claim_id = $1
        ORDER BY level")
        .bind(claim_id).fetch_all(pool).await?)
}
```

## State machine extension

The existing
`ClaimStatus::can_transition_to` (from
`2026-08-14-expense-reimbursement`) gains two transitions:

- `submitted → partially_approved`
- `partially_approved → partially_approved` (self; same level
  by another user is idempotent).
- `partially_approved → fully_approved` (terminal; replaces
  `submitted → approved`).
- `fully_approved → paid` (unchanged).

The transition is computed inside the transaction:

```rust
let required = required_levels(&mut *tx, ledger_id, total).await?;
let recorded = recorded_levels(&mut *tx, claim_id).await?;
let next = if required.iter().all(|l| recorded.contains(l)) {
    ClaimStatus::FullyApproved
} else {
    ClaimStatus::PartiallyApproved
};
if next == ClaimStatus::FullyApproved {
    post_approval(&mut tx, &claim).await?;     // GL fires once
}
```

## Approver lookup

For each required level, the system picks an approver from
users with `role >= approver_role` on the ledger. If multiple
qualify, the first (lexicographic by user id) is the expected
approver. The handler accepts any qualified user; the
`approver_id` stored is whoever actually approves.

```rust
pub async fn eligible_approvers(
    pool: &PgPool, ledger_id: Uuid, min_role: Role,
) -> AppResult<Vec<Uuid>> {
    // ledger_shares (migration 0008) + owners
    sqlx::query_scalar(
        "SELECT user_id FROM ledger_shares
         WHERE ledger_id = $1 AND role >= $2
         UNION SELECT id FROM users WHERE id =
            (SELECT owner_id FROM ledgers WHERE id = $1)
         ORDER BY 1"
    )
    .bind(ledger_id).bind(min_role)
    .fetch_all(pool).await
}
```

## Tests

### Unit

- `required_levels` empty → `[1]`.
- `required_levels` with two matching policies → both levels.
- `recorded_levels` is sorted distinct.

### Integration

- `http_claim_above_threshold_requires_two_levels`.
- `http_two_admin_approves_first_level_only` — claim stays
  `partially_approved`.
- `http_two_levels_approved_moves_to_fully_approved_and_posts_gl`
  — GL posts exactly once.
- `http_author_cannot_self_approve_in_shared_ledger` —
  403 with documented body.
- `http_admin_sees_level_breakdown` — admin sees per-level
  status, author sees aggregate only.

### Property

- `prop_required_levels_sorted_distinct` — random totals
  always produce sorted-distinct level lists.

## References

- SAP Concur "Approval Routing"
- Zoho Expense "Policies" (multi-level)
- Frappe HRMS Approval Workflow
