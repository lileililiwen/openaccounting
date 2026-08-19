#!/usr/bin/env bash
# Build a reproducible release binary outside of CI.
#
# See openspec/changes/d4-reproducible-builds/specs/reproducible-builds/spec.md
# and docs/release-verification.md.
#
# Requirements:
#   * rustup with the toolchain pinned in rust-toolchain.toml
#   * Optionally: `cosign` (https://docs.sigstore.dev/cosign/) for signing
#
# Usage:
#   scripts/release.sh                  # build + hash
#   scripts/release.sh --sign           # build + sign with cosign keyless OIDC
#   scripts/release.sh --output PATH    # custom output path

set -euo pipefail

OUTPUT="${OUTPUT:-target/release/openaccounting}"
SIGN=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --sign)        SIGN=1; shift ;;
    --output)      OUTPUT="$2"; shift 2 ;;
    --output=*)    OUTPUT="${1#--output=}"; shift ;;
    -h|--help)
      sed -n '2,16p' "$0"
      exit 0
      ;;
    *)             echo "unknown arg: $1" >&2; exit 64 ;;
  esac
done

# Reproducibility env (mirrors .github/workflows/release.yml).
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1700000000}"
export RUSTFLAGS="${RUSTFLAGS:- -C link-arg=-Wl,--build-id=none -C debuginfo=0}"
export CARGO_INCREMENTAL=0

mkdir -p "$(dirname "$OUTPUT")"

echo "==> building release binary"
cargo build --release --locked

echo "==> verifying reproducibility"
TMP="$(mktemp)"
cp "$OUTPUT" "$TMP"
cargo build --release --locked

H1="$(sha256sum "$TMP"   | awk '{print $1}')"
H2="$(sha256sum "$OUTPUT" | awk '{print $1}')"

echo "build #1: $H1"
echo "build #2: $H2"
if [ "$H1" != "$H2" ]; then
  echo "ERROR: build is NOT reproducible" >&2
  rm -f "$TMP"
  exit 1
fi
echo "reproducible ✓"
rm -f "$TMP"

if [ "$SIGN" -eq 1 ]; then
  if ! command -v cosign >/dev/null 2>&1; then
    echo "ERROR: --sign requested but cosign is not installed" >&2
    exit 127
  fi
  echo "==> signing with cosign (keyless OIDC)"
  COSIGN_EXPERIMENTAL=1 cosign sign-blob \
    --output-signature "${OUTPUT}.sig" \
    --output-certificate "${OUTPUT}.cert" \
    --bundle "${OUTPUT}.bundle" \
    "$OUTPUT"
  echo "signature bundle: ${OUTPUT}.bundle"
fi

echo "==> done"
echo "binary:  $OUTPUT"
echo "sha256:  $H2"