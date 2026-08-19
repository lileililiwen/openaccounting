# Reproducible Builds and Signed Releases

## Why

The release profile in `Cargo.toml` is `opt-level = "z"`, `lto =
true`, `codegen-units = 1`, `strip = true`. That's the right shape for
size but not necessarily for reproducibility — source info, build
timestamps, and toolchain paths leak into the binary. Reproducible
builds + cosign signatures turn the binary into something a security-
conscious org can verify.

## What Changes

- Add `SOURCE_DATE_EPOCH` and `RUSTFLAGS="-C link-arg=-Wl,--build-id=none"`
  in CI.
- Add a GitHub Actions job that builds the binary in a pinned Docker
  image, computes SHA256, and publishes it to GitHub Releases with a
  cosign signature.
- The README documents the verification command.

## Capabilities

### New Capabilities

- `reproducible-builds`: Reproducible + signed release binaries.

## Impact

**New files:**
- `.github/workflows/release.yml`.
- `scripts/release.sh`.
- `docs/release-verification.md`.
