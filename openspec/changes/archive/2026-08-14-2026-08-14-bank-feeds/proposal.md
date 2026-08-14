# Add Live Bank Feeds (Plaid / GoCardless / Salt Edge / SimpleFIN)

## Why

Manual CSV / OFX import will always lag reality by days. Live
bank feeds let a user link an account at their bank and have
transactions flow in nightly, matched to their ledger
automatically. Firefly III has had this for years via
GoCardless, SimpleFIN, Sophtron, and Salt Edge; Maybe
(formerly Assist) used Plaid. ezBookkeeping has none.

For non-US markets Plaid coverage is spotty; the spec
therefore ships a **provider trait** with adapters for
Plaid (US/CA), GoCardless (EU/UK), Salt Edge (global),
SimpleFIN (US free), and a stub `manual` adapter so the
self-hosted install works without external dependencies.

## What Changes

- New table `bank_feed_links`
  (`id, ledger_id, provider, institution_id, account_id_at_provider,
   account_id_in_ledger, status, access_token_encrypted,
   refresh_token_encrypted, cursor, last_synced_at`).
- New module `src/bank_feeds/` with `Provider` trait and
  adapters: `plaid`, `gocardless`, `salt_edge`, `simplefin`,
  `manual`.
- New routes:
  - `GET  /ledgers/{id}/bank-feeds`            — list links
  - `POST /ledgers/{id}/bank-feeds/plaid/link` — OAuth handshake
  - `GET  /ledgers/{id}/bank-feeds/{link_id}/sync`
  - `POST /ledgers/{id}/bank-feeds/{link_id}/unlink`
- New background sync worker triggered by:
  - cron at 02:00 UTC daily (configurable),
  - manual "Sync now" button.
- New env var `BANK_FEEDS_ENCRYPTION_KEY` (32 bytes, base64)
  used to encrypt provider access tokens at rest with
  AES-GCM.
- Tokens are stored encrypted; the encryption key never leaves
  the server process.

## Capabilities

### New Capabilities

- `bank-feeds` — provider-pluggable live transaction sync.

## Impact

- **New files:**
  - `migrations/0024_add_bank_feed_links.sql`
  - `src/bank_feeds/mod.rs` (Provider trait)
  - `src/bank_feeds/plaid.rs`, `gocardless.rs`,
    `salt_edge.rs`, `simplefin.rs`, `manual.rs`
  - `src/handlers/bank_feeds.rs`
  - `src/templates/bank_feeds.rs`
  - `templates/bank_feeds/{list,link_plaid,show}.html`
  - `src/workers/sync.rs` (background worker)
  - `tests/integration/bank_feeds.rs`
  - `tests/fixtures/bank_feeds/`
- **Modified files:**
  - `Cargo.toml` — `aes-gcm = "0.10"`, `base64 = "0.22"`,
    `reqwest = { features = ["json", "rustls-tls"] }`,
    `tokio-cron-scheduler = "0.13"`.
  - `src/config.rs` — add `bank_feeds_encryption_key`,
    `bank_feeds_sync_schedule`.
  - `src/main.rs` — start sync worker on boot.
  - `.env.example` — document the new env vars.
  - `templates/partials/_nav.html` — add "Bank Feeds" link.

## Non-Goals

- WeChat Pay / Alipay live sync (those platforms do not
  expose developer APIs for personal accounts). File import
  only.
- Real-time push (webhook-based instant sync). Nightly batch
  only in v1.
- Custom provider plugins from end users. Provider list is
  closed.
