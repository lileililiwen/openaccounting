# Design

## Decisions

### Make current source and accepted specs mutually consistent

The implementation and archived accepted specs are the evidence base. README claims must distinguish shipped capabilities, experimental/optional capabilities, and deferred work. Current architecture specs must describe `src/lib.rs` as the composition root for router construction while preserving the thin `src/main.rs` startup shell.

### Keep historical changes immutable in meaning

Archived changes remain historical records. The current `openspec/specs/` capability files and top-level documentation receive the durable corrections. Historical text is changed only when it contains a broken current link or an objectively incorrect repository identity.

### Risks and mitigations

- Removing old non-goals can overstate production readiness; mitigate by labeling alpha, experimental, provider-dependent, and unverified capabilities explicitly.
- Updating specs can expose implementation drift; mitigate by converting each mismatch into a follow-up change rather than silently changing behavior.

## Verification

Run strict OpenSpec validation, scan current docs for contradictory feature/non-goal claims, verify every referenced path exists, and compare architecture statements with the module layout and `Cargo.toml`.
