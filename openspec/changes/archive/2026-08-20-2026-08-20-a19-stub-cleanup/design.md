# Stub Cleanup — Design

## Approach

Three independent fixes, each turning an existing implemented path into
a reachable, working one.

## Decisions

- **API tokens: mount + link.** The router already exists
  (`account_api_tokens::router()`); it just needs `.merge()` in the
  protected router and a link from the account page. WHY: zero new
  logic, immediate value. Mitigation: the token page shows the
  plaintext once, matching the existing issue flow.

- **Plaid webhook: real HMAC-SHA256.** Plaid signs webhooks with
  HMAC-SHA256 over the raw request body using the webhook secret; the
  `Plaid-Verification` header is `v1: <hex mac>`. Implement using the
  `hmac` + `sha2` crates (already in Cargo.toml). WHY: the check is a
  security boundary; a stub defeats it. Mitigation: constant-time
  comparison; when `PLAID_WEBHOOK_SECRET` is empty the check stays
  disabled (documented).

- **Transfer form: reuse the existing create handler.** The form posts
  the same `TransferForm` the create handler already validates. The page
  renders the user's ledgers (grouped by account) with client-side
  filtering for the account selects. WHY: no server-side new logic, the
  create path is tested. Mitigation: the transfer list links are added
  to the transactions list header.

## Data Flow

1. `lib.rs` merges the API-token router; the account page links to
   `/account/api-tokens`.
2. `webhook_plaid` calls the real `verify_plaid_signature`.
3. `/transfers/inter-ledger` renders a form; submit posts to the same
   route's POST handler which creates the two transactions.

## Risks / Trade-offs

- Inter-ledger transfers require owning two ledgers. Mitigation: the
  form requires at least two ledgers and explains that.
- The Plaid header format may vary (`v1:` is the documented format).
  Mitigation: only the `v1:` scheme is accepted; unknown schemes fail
  closed (return false → 401).
