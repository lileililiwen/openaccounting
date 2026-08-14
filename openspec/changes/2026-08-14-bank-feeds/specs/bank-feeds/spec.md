# bank-feeds Specification

## Purpose

Define the pluggable provider interface and the lifecycle for
live bank-feed synchronization.

## Requirements

### Requirement: Provider Trait

```rust
#[async_trait]
pub trait BankFeedProvider: Send + Sync {
    fn id(&self) -> &'static str;            // "plaid", etc.
    fn label(&self) -> &'static str;         // "Plaid"
    async fn list_institutions(&self) -> Result<Vec<Institution>>;
    async fn exchange_public_token(&self, public_token: &str)
        -> Result<AccessTokens>;
    async fn list_accounts(&self, access: &AccessTokens)
        -> Result<Vec<ProviderAccount>>;
    async fn fetch_transactions(&self, access: &AccessTokens,
        cursor: Option<&str>) -> Result<TransactionPage>;
}
```

v1 ships five adapters: `plaid`, `gocardless`, `salt_edge`,
`simplefin`, `manual`. Each adapter is selected via
`BANK_FEED_PROVIDER` env var; the binary uses one provider at
a time.

#### Scenario: Default provider is manual

- **WHEN** `BANK_FEED_PROVIDER` is unset or `manual`
- **THEN** the `Manual` adapter is used (a no-op that exposes
  a "no live sync" UI); the rest of the routes still work for
  CSV / OFX import.

### Requirement: Link Lifecycle

`POST /ledgers/{id}/bank-feeds/{provider}/link` begins the
OAuth / public-token handshake. The provider returns a redirect
URL; the user authorizes; the provider posts back to a
provider-specific callback route with the public token; the
handler exchanges it for `access_token` + `refresh_token`,
encrypts both, stores them in `bank_feed_links`, and 303s to
`/ledgers/{id}/bank-feeds/{link_id}`.

#### Scenario: Plaid link round-trip

- **WHEN** the user clicks "Link Plaid account" and completes
  Plaid Link in the browser
- **THEN** `bank_feed_links` gains a row with `provider='plaid',
  status='active'` and the access token is AES-GCM encrypted.

### Requirement: Encrypted Token Storage

Provider access and refresh tokens SHALL be stored encrypted
at rest using AES-256-GCM. The encryption key is read from
`BANK_FEEDS_ENCRYPTION_KEY` (32-byte base64). If the env var
is missing or shorter than 32 bytes, startup fails with
`BANK_FEEDS_ENCRYPTION_KEY must be a 32-byte base64 string.`.

Tokens are never returned in any HTTP response. The
`bank_feed_links` row exposes only `provider`, `institution_id`,
`account_id_at_provider`, `status`, `last_synced_at`, and a
short `link_nickname` (user-editable).

#### Scenario: Token is encrypted in DB

- **WHEN** the link is created with `access_token="access-abc"`
- **THEN** the `access_token_encrypted` column is base64
  ciphertext, and a raw `SELECT access_token_encrypted FROM
  bank_feed_links` does not contain the substring "abc".

### Requirement: Sync Worker

A background tokio task SHALL run every N hours (default 24,
configurable via `BANK_FEEDS_SYNC_INTERVAL_HOURS`). For each
`active` link it:

1. Calls `provider.fetch_transactions(cursor=link.cursor)`.
2. For each new transaction, applies the same dedup
   fingerprint from `data-import`.
3. On success, updates `link.cursor` and `link.last_synced_at`.
4. Writes an audit row `bank_feed.sync.success` with the
   count, or `bank_feed.sync.failed` with the error message
   if the provider returns an error.

#### Scenario: Daily sync adds new transactions

- **WHEN** the worker runs at 02:00 UTC and the provider
  returns 3 new transactions
- **THEN** 3 new `transactions` rows are added (each with two
  postings against the linked ledger account and the default
  cash account), `bank_feed_links.cursor` is updated, and
  `last_synced_at` is set to the run timestamp.

### Requirement: Manual Sync

`POST /ledgers/{id}/bank-feeds/{link_id}/sync` triggers an
immediate sync (the worker also runs in-line). The response
is `303 See Other` to the link detail page with a flash
message `Synced N transactions.`.

#### Scenario: Manual sync succeeds

- **WHEN** the user clicks "Sync now" on a link
- **THEN** the page reloads with the flash `Synced 5
  transactions.` and `last_synced_at` is updated.

### Requirement: Unlink

`POST /ledgers/{id}/bank-feeds/{link_id}/unlink` sets
`status='unlinked'`, deletes the encrypted tokens, and 303s to
the bank-feeds list page. Historical transactions imported
via this link are NOT deleted (they remain in the ledger
with a `bank_feed_link_id` reference for traceability).

#### Scenario: Unlink preserves imported transactions

- **WHEN** the user unlinks an account that imported 10
  transactions
- **THEN** the 10 `transactions` rows remain, each with
  `bank_feed_link_id = <id>` and `provider='plaid'`. The link
  row's tokens are gone.

### Requirement: Webhook Receivers (Plaid)

For providers that support webhooks (Plaid), the binary SHALL
expose `POST /webhooks/{provider}` that verifies the provider
signature, enqueues a sync job for the affected link, and
returns `200 OK` immediately. The signature verification uses
the same `BANK_FEEDS_ENCRYPTION_KEY` for HMAC.

#### Scenario: Plaid webhook accepted

- **WHEN** Plaid POSTs `TRANSACTIONS_SYNC_UPDATES_AVAILABLE`
  to `/webhooks/plaid` with a valid signature
- **THEN** the response is `200 OK` and the affected link's
  sync is enqueued within 60 seconds.
