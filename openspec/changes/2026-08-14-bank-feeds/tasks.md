# Bank Feeds — Tasks

## 1. Testing

- [ ] 1.1 Unit: `TokenCipher::seal` then `open` round-trips.
- [ ] 1.2 Unit: `TokenCipher::seal` of `access-abc` does not
      contain the substring `access-abc`.
- [ ] 1.3 Integration: `http_link_plaid_stores_encrypted_token`.
- [ ] 1.4 Integration: `http_sync_creates_transactions` with
      mocked provider.
- [ ] 1.5 Integration: `http_unlink_preserves_transactions`.
- [ ] 1.6 Integration: `http_webhook_plaid_rejects_bad_signature`.
- [ ] 1.7 Integration: `http_manual_provider_does_not_call_external_api`.
- [ ] 1.8 Property: `prop_token_cipher_is_bijective` for 1000
      random plaintexts.
- [ ] 1.9 Integration:
      `http_missing_encryption_key_fails_startup` — start the
      binary without the env var, expect a clear error.

## 2. Implementation

- [ ] 2.1 Add `aes-gcm = "0.10"`, `base64 = "0.22"`,
      `reqwest = { features = ["json", "rustls-tls"] }`,
      `tokio-cron-scheduler = "0.13"` to `Cargo.toml`.
- [ ] 2.2 Migration `0024_add_bank_feed_links.sql`.
- [ ] 2.3 `src/bank_feeds/mod.rs` — `Provider` trait,
      `BankFeedError`.
- [ ] 2.4 `src/bank_feeds/plaid.rs`.
- [ ] 2.5 `src/bank_feeds/gocardless.rs`.
- [ ] 2.6 `src/bank_feeds/salt_edge.rs`.
- [ ] 2.7 `src/bank_feeds/simplefin.rs`.
- [ ] 2.8 `src/bank_feeds/manual.rs` — no-op stub.
- [ ] 2.9 `src/bank_feeds/crypto.rs` — `TokenCipher`.
- [ ] 2.10 `src/handlers/bank_feeds.rs` — list / link / sync /
      unlink / webhook routes.
- [ ] 2.11 `src/workers/sync.rs` — background sync loop,
      started from `main.rs` on boot.
- [ ] 2.12 `src/config.rs` — `bank_feeds_encryption_key`,
      `bank_feeds_sync_interval_hours`, `bank_feed_provider`.
- [ ] 2.13 `.env.example` — document the new env vars.
- [ ] 2.14 `src/main.rs` — wire 5 routes + the sync worker.
- [ ] 2.15 Templates
      `templates/bank_feeds/{list,link_plaid,show}.html`.

## 3. Validation

- [ ] 3.1 `openspec validate bank-feeds` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke (Plaid sandbox): link a sandbox
      account, sync, assert 0+ transactions imported, unlink,
      assert tokens removed.
- [ ] 3.6 `openspec archive bank-feeds`.
