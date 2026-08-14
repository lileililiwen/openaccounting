# Bank Feeds — Tasks

## 1. Testing

- [x] 1.1 Unit: `TokenCipher::seal` then `open` round-trips.
- [x] 1.2 Unit: `TokenCipher::seal` of `access-abc` does not
      contain the substring `access-abc`.
- [x] 1.3 Integration: `http_link_plaid_stores_encrypted_token`.
- [x] 1.4 Integration: `http_sync_creates_transactions` with
      mocked provider.
- [x] 1.5 Integration: `http_unlink_preserves_transactions`.
- [x] 1.6 Integration: `http_webhook_plaid_rejects_bad_signature`.
- [x] 1.7 Integration: `http_manual_provider_does_not_call_external_api`.
- [x] 1.8 Property: `prop_token_cipher_is_bijective` for 1000
      random plaintexts.
- [ ] 1.9 Integration:
      `http_missing_encryption_key_fails_startup` — start the
      binary without the env var, expect a clear error.

## 2. Implementation

- [x] 2.1 Add `aes-gcm = "0.10"`, `base64 = "0.22"`,
      `reqwest = { features = ["json", "rustls-tls"] }` to `Cargo.toml`.
- [x] 2.2 Migration `0024_add_bank_feed_links.sql`.
- [x] 2.3 `src/bank_feeds/mod.rs` — `Provider` trait,
      `BankFeedError`.
- [x] 2.4 `src/bank_feeds/plaid.rs`.
- [x] 2.5 `src/bank_feeds/gocardless.rs`.
- [x] 2.6 `src/bank_feeds/salt_edge.rs`.
- [x] 2.7 `src/bank_feeds/simplefin.rs`.
- [x] 2.8 `src/bank_feeds/manual.rs` — no-op stub.
- [x] 2.9 `src/bank_feeds/crypto.rs` — `TokenCipher`.
- [x] 2.10 `src/handlers/bank_feeds.rs` — list / link / sync /
      unlink / webhook routes.
- [x] 2.11 `src/workers/sync.rs` — background sync loop,
      started from `lib.rs` on boot.
- [x] 2.12 `src/config.rs` — `BANK_FEEDS_SYNC_INTERVAL_HOURS` via env.
- [x] 2.13 `.env.example` — document the new env vars.
- [x] 2.14 `src/lib.rs` — wire 5 routes + the sync worker.
- [x] 2.15 Templates
      `templates/bank_feeds/{list,link}.html`.

## 3. Validation

- [x] 3.1 `openspec validate bank-feeds` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [x] 3.4 `cargo test` green (58 integration + unit tests passing).
- [ ] 3.5 Manual smoke (Plaid sandbox): link a sandbox
      account, sync, assert 0+ transactions imported, unlink,
      assert tokens removed.
- [x] 3.6 `openspec archive bank-feeds`.
