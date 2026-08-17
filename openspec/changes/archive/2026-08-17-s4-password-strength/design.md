# ## Context

`src/auth/handlers.rs:105` and `src/auth/mod.rs:150` enforce length ≥ 8.
The README/Agents.md also say "≥ 8". This change moves the floor to 12
and adds a deny list.

## Goals / Non-Goals

**Goals:**
- Prevent weak and breached passwords.
- Default-offline check (no third-party call); optional online via feature
  flag.
- Zero information leakage.

**Non-Goals:**
- Mandatory online HIBP (some deployments have no egress).
- Password rotation policy (deprecated by NIST).
- Per-account entropy meter UI (separate change).

## Decisions

- **Bundled deny list**: the top-100k from SecLists, gzipped, ~250 KB
  uncompressed. Loaded once at startup into a `HashSet<String>`.
- **HIBP k-anonymity**: send `SHA1(password)[0..5]` (5 bytes hex = 10
  chars); receive ~800 suffixes; check locally.
- **Fail-open on API error**: HIBP downtime should not block sign-ups.

## Risks / Trade-offs

- **Common-password list size**: 100k is small enough to load into memory
  but large enough to catch the bulk of weak choices. The full HIBP list is
  800M+ entries — out of scope.
- **Backwards compatibility**: existing users with 8-11 char passwords can
  still log in. The new policy applies at next password change only.
