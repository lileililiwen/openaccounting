# ## Context

`src/lib.rs:118` hard-codes `with_secure(false)`. The auth spec says
production MUST be `true`. There is no enforcement.

## Goals / Non-Goals

**Goals:**
- Default-secure in production; default-insecure in development.
- Loud, blocking failure if production tries to serve over plain HTTP.

**Non-Goals:**
- Automatic Let's Encrypt (separate change).
- Trust-store management.

## Decisions

- `APP_ENV` parsed in `config.rs` with strict enum semantics.
- Production startup validates that `APP_HOST` parses to a URL with scheme
  `https` unless `--allow-insecure-cookies` is passed.
- Tests run with `APP_ENV=test`, which permits both `Secure` and non-`Secure`
  cookies for the `TestServer` fixture (it uses `tower::ServiceExt::oneshot`
  which doesn't go through TLS).

## Risks / Trade-offs

- **First-deploy confusion**: an operator who hasn't terminated TLS will
  get a clear startup error. Documented in README.
- **Behind reverse proxy**: if the proxy terminates TLS and forwards to
  the app over HTTP, the operator MUST pass `--allow-insecure-cookies` AND
  set a `X-Forwarded-Proto`-aware flag. We document this but do not
  auto-detect (HTTP host header injection would be a risk).
