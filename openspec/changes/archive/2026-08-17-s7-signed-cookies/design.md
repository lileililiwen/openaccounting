# ## Context

`app_secret` is unused today. `tower-sessions` has a `signed` feature.

## Goals / Non-Goals

**Goals:**
- Detect tampering.
- Allow key rotation.

**Non-Goals:**
- Per-session encryption of the payload (the payload is opaque session id;
  the server stores the data).

## Decisions

- `tower-sessions` `signed` feature on, key derived from `APP_SECRET`.
- For rotation, we use the `with_key_fallback` API if available, or pass
  a `Vec<Key>` if not.

## Risks / Trade-offs

- Larger cookies (~32 bytes of MAC).
- Key rotation policy must be documented.
