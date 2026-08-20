# Stub Cleanup

## Why

Several features are implemented but unreachable or stubbed:

1. **API tokens are never exposed.** The token page, issue, and revoke
   handlers and templates are complete, but the router is never mounted
   in `lib.rs` — users cannot create an API token, so the REST API is
   unusable in practice.
2. **The Plaid webhook signature check is a stub** that always returns
   true. When `PLAID_WEBHOOK_SECRET` is set the handler intends to
   verify the HMAC, but the helper returns `true` unconditionally.
3. **The inter-ledger transfer form is a placeholder.** The route
   exists but renders plain text ("lives here in a follow-up UI
   change"), while the create handler behind it is fully implemented.

## What Changes

- Mount the **API tokens router** and link it from the account page.
- Implement the **Plaid webhook signature verification** (HMAC-SHA256,
  `v1:` header format) using the already-present `hmac`/`sha2` deps.
- Replace the inter-ledger transfer **placeholder page** with a real
  form (from/to ledger + account, amount, date, description, optional
  fee) that posts to the existing create endpoint, and link it from the
  transactions list.

## Capabilities

- `api-token-ui`: issue/revoke REST API tokens from the account page.
- `plaid-webhook-verification`: real HMAC-SHA256 webhook signature check.
- `inter-ledger-transfer-form`: a working transfer form.

## Non-Goals

- Real APNs/FCM push delivery (the notifiers are log-only; wiring actual
  Apple/Google push is an ops concern for a self-hosted install).
- Investment-lot data entry (the reports exist but nothing ingests lots).
- The `manual` bank feed provider (intentionally a no-op for
  zero-config installs).
