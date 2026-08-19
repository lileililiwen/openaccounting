# ## Context

Build reproducibility is a single-day investment for a measurable trust
gain.

## Goals / Non-Goals

**Goals:**
- Reproducible + signed.

**Non-Goals:**
- SLSA Level 3 (separate).

## Decisions

- Use `cargo-chef` for cacheable Docker layers.
- Pin via `rust-toolchain.toml`.
- cosign keyless signing with GitHub OIDC.
