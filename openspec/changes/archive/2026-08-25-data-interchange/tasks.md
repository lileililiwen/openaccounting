# 1. Testing

- [x] 1.1 Unit: OFX v1 SGML fixture parses (date, amount, payee, fitid); OFX v2 XML likewise.
- [x] 1.2 Unit: QIF fixture with `D07/01'26` dates resolves under both US/EU date orders with override.
- [x] 1.3 Unit: CAMT.053 fixture (2 banks' real files) → n lines, balances sum to opening+closing delta.
- [x] 1.4 Unit: MT940 `:61:`/`:86:` fixture → lines with reference + payee extraction.
- [x] 1.5 Integration: same OFX twice → second run all-duplicates, zero inserts.
- [x] 1.6 HTTP: renamed-extension files still parse; EUR rows into USD account error per-row.
- [x] 1.7 Integration: two confirmations build alias with hit_count=2; suggestion endpoint ranks it first for prefix "acme".
- [x] 1.8 Unit: confidence decay — 5 old hits < threshold after 12 months idle; fresh 3 hits ≥ threshold.
- [x] 1.9 HTTP: retroactive apply dry-run shows diff; confirm writes batch; closed-period overlap → 409 and no partial writes.
- [x] 1.10 Property: 200 random alias/confirm sequences never suggest an account the ledger doesn't contain.

# 2. Implementation

- [x] 2.1 Extract statement-line normalizer from CSV importer.
- [x] 2.2 Parsers: `ofx.rs`, `qif.rs`, `camt.rs`, `mt940.rs` + content sniffer.
- [x] 2.3 Wire into `/reconcile/{id}/import`; dedupe flags in UI.
- [x] 2.4 Migration `payee_aliases`; capture on match-confirm + payee edit.
- [x] 2.5 Suggestion endpoint + entry-form autocomplete; import preview pre-fill.
- [x] 2.6 Retroactive apply route + dry-run diff UI.

# 3. Validation

- [x] 3.1 `openspec validate data-interchange`.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets --features test-support`.
- [x] 3.3 `cargo test --features test-support`.
