# Audit Chain Treatment Record

**Workflow:** Append-only hash chain for transaction integrity
**Jurisdiction:** generic
**Scope:** Append-only hash chain for transaction integrity

## Recognition timing

Audit entries are created automatically for every data-modifying
operation (INSERT, UPDATE on transactions, postings, accounts,
etc.). The hash chain is backfilled on startup if incomplete.

## Posting treatment

Each audit entry carries: ledger_id, actor_id, action,
entity_type, entity_id, old_value (JSON diff), new_value,
and a SHA-256 hash of the previous hash concatenated with the
canonical row bytes.

## Rounding

Not applicable (no financial calculations in the audit chain).

## Reversal / void

Audit entries are append-only. Reversals create new audit
entries recording the reversal action. The original audit
entry is preserved.

## Report inclusion

The audit chain is used for tamper-evidence verification,
not for financial reporting. The verify endpoint walks the
chain and reports any broken links.

## Assumptions

- SHA-256 hash chain.
- No external timestamping service.
- Chain is backfilled on startup (idempotent).
- Advisory locks prevent concurrent chain appends.

## Non-compliance disclaimer

The audit chain provides tamper-evidence but is NOT a
legally binding digital signature. It does not replace
formal audit controls required by jurisdiction-specific
regulations. Use as a supplementary integrity check only.
