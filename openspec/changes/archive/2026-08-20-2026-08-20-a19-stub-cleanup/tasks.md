# Stub Cleanup — Tasks

## 1. Testing

- [x] 1.1 Integration test: GET `/account/api-tokens` (logged in) renders the token page; POST create issues a token and shows it once; revoke marks it revoked.
- [x] 1.2 Unit test: `verify_plaid_signature` accepts a correct `v1:` HMAC-SHA256 signature and rejects a wrong body/secret, an unknown scheme, and a malformed header.
- [x] 1.3 Integration test: POST an inter-ledger transfer through the form creates two transactions and an `inter_ledger_transfers` link (the create path already exists).
- [x] 1.4 Integration test: the transfer form GET renders with the user's ledgers listed.

## 2. Implementation

- [x] 2.1 Mount `account_api_tokens::router()` in `lib.rs`; add an "API tokens" link to the account page.
- [x] 2.2 Implement `verify_plaid_signature` with `hmac`/`sha2` (v1 scheme, hex compare in constant time).
- [x] 2.3 Add `TransfersPage` template struct + `templates/transfers/inter_ledger.html` form; replace the placeholder `new_page` handler.
- [x] 2.4 Link the transfer form from the transactions list header.
- [x] 2.5 Add integration + unit tests.

## 3. Documentation

- [x] 3.1 Note the REST API tokens and inter-ledger transfers in the README.
