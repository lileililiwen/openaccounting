# Bank Feeds — Design

## Encryption

```rust
// src/bank_feeds/crypto.rs
pub struct TokenCipher {
    key: [u8; 32],     // BANK_FEEDS_ENCRYPTION_KEY
}

impl TokenCipher {
    pub fn seal(&self, plaintext: &str) -> String {
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        use aes_gcm::aead::{Aead, KeyInit};
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Nonce::from_slice(&rand::random::<[u8;12]>());
        let ct = cipher.encrypt(nonce, plaintext.as_bytes()).unwrap();
        format!("{}:{}", base64::encode(nonce), base64::encode(ct))
    }
    pub fn open(&self, blob: &str) -> Option<String> {
        // parse "nonce:ct", decrypt
    }
}
```

The key is loaded once at startup from
`BANK_FEEDS_ENCRYPTION_KEY`. If absent, startup fails.

## Provider implementations

### Plaid

`src/bank_feeds/plaid.rs` uses `reqwest` against
`https://production.plaid.com` (configurable to `sandbox` /
`development` via `PLAID_ENV`). Public token exchange
endpoint: `/item/public_token/exchange`. Transactions sync:
`/transactions/sync`. The cursor is stored in
`bank_feed_links.cursor`.

### GoCardless

`src/bank_feeds/gocardless.rs` — uses the GoCardless Bank
Account Data API (`bankaccountdata.gocardless.com`). The
`requisition` flow yields a link_id; we map to
`bank_feed_links`. Transactions come from
`/accounts/{id}/transactions`.

### Salt Edge

`src/bank_feeds/salt_edge.rs` — uses the Salt Edge Connect
API. The `connect_sessions/create` flow yields a `connect_url`;
the user authorizes; the provider posts a webhook to our
`/webhooks/salt_edge` route.

### SimpleFIN

`src/bank_feeds/simplefin.rs` — SimpleFIN is the simplest of
the four. The user visits `https://beta.simplefin.org`,
generates a token, pastes it in the link form. We POST to
`https://beta.simplefin.org/simplefin/accounts` with HTTP Basic
auth (`token:token`).

### Manual

`src/bank_feeds/manual.rs` — a no-op stub. The provider
trait methods return `Ok(vec![])` and the routes still work
for CSV / OFX / WeChat / Alipay imports. This keeps a
zero-config self-host install viable without third-party
credentials.

## Sync worker

```rust
// src/workers/sync.rs
pub async fn run_sync_loop(state: AppState) {
    let interval = Duration::from_secs(
        state.config.bank_feeds_sync_interval_hours * 3600);
    loop {
        for link in active_links(&state.pool).await? {
            if let Err(e) = sync_one(&state, link.id).await {
                warn!("sync failed for {}: {}", link.id, e);
            }
        }
        tokio::time::sleep(interval).await;
    }
}
```

Each `sync_one` call:

1. Decrypts the access token.
2. Calls `provider.fetch_transactions(cursor)`.
3. For each new transaction: run dedup, create one
   transaction + two postings (DR user-chosen account / CR
   default cash account).
4. Updates `cursor` and `last_synced_at`.

## Webhook signature verification (Plaid)

```rust
pub fn verify_plaid(body: &[u8], header: &str, secret: &str)
    -> bool
{
    use hmac::{Hmac, Mac};
    use sha256::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .unwrap();
    mac.update(body);
    let expected = mac.finalize().into_bytes();
    let sent = base64::decode(header).unwrap_or_default();
    expected.as_slice() == sent
}
```

## Tests

### Unit

- `TokenCipher::seal` then `open` round-trips.
- `TokenCipher::seal` of `access-abc` does not contain the
  substring `access-abc` in the ciphertext.
- Plaid client builds the correct request URL.

### Integration

- `http_link_plaid_stores_encrypted_token` — POST link →
  row in DB, raw token never readable.
- `http_sync_creates_transactions` — sync with mocked
  provider → N transactions.
- `http_unlink_preserves_transactions` — historical txns
  remain.
- `http_webhook_plaid_rejects_bad_signature` — 401 on bad
  signature, 200 on good.
- `http_manual_provider_does_not_call_external_api`.

### Property

- `prop_token_cipher_is_bijective` — for 1000 random
  plaintexts, `open(seal(p)) == p`.

## References

- Plaid API docs (`plaid.com/docs`)
- GoCardless Bank Account Data API docs
- Salt Edge Connect API docs
- SimpleFIN protocol (`simplefin.org`)
- Firefly III Data Importer
