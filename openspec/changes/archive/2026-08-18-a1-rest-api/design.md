# ## Context

Zero JSON surface today. The HTML handlers are excellent and well tested;
the API is a thin JSON adapter that re-uses the same domain helpers.

## Goals / Non-Goals

**Goals:**
- Cover the high-value endpoints.
- Bearer-token auth (no sessions, no cookies).
- Stable wire format.

**Non-Goals:**
- GraphQL.
- Webhooks (separate; piggy-back on bank-feed webhooks).
- OData query language.

## Decisions

- One `api_token` row per token, with last-used timestamp; revoked tokens
  stay in the table for audit.
- All endpoints use the same domain helpers as the HTML handlers — no
  duplicated SQL.
- Problem-details error type is a small struct that maps to RFC 7807.

## Risks / Trade-offs

- API surface becomes a public commitment. Document versioning policy.
- More endpoints means more tests. We add integration tests for each
  public endpoint.
