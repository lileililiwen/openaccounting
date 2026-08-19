# Release Verification

OpenAccounting ships signed, reproducible release binaries. Every
binary attached to a [GitHub Release][releases] is:

1. **Reproducible** — two consecutive builds of the same source commit
   produce a bit-identical binary (verified by CI before signing).
2. **Signed** — every binary is signed with [`cosign`][cosign] using
   keyless OIDC tied to this GitHub repository.

[releases]: https://github.com/anomalyco/openaccounting/releases
[cosign]: https://docs.sigstore.dev/cosign/

---

## 1. Download

Download the three artifacts attached to the release:

| File                  | Purpose                                     |
| --------------------- | ------------------------------------------- |
| `openaccounting`      | The release binary itself.                  |
| `openaccounting.bundle` | Signature bundle (contains `.sig` + `.cert`). |
| `openaccounting.sig`  | Signature (already inside the bundle).      |
| `openaccounting.cert` | Signing certificate (already in the bundle).|

You can download all of them via the GitHub CLI:

```sh
gh release download v0.1.0
```

## 2. Verify the SHA256

The CI run that produced the release prints the binary's SHA256.
Compare it to what you computed locally:

```sh
sha256sum openaccounting
```

The hash must match exactly.

## 3. Verify the signature

```sh
cosign verify-blob \
  --bundle openaccounting.bundle \
  --certificate-identity-regexp 'https://github.com/anomalyco/openaccounting/.github/workflows/release.yml@refs/tags/v0.1.0' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  openaccounting
```

Replace `v0.1.0` with the tag you downloaded.

The output must end with:

```
Verified OK
```

If verification fails, **do not run the binary**. Open an issue and
reference the tag.

---

## Reproducing a build locally

If you have the Rust toolchain installed (the version pinned in
`rust-toolchain.toml`), you can rebuild the exact same binary:

```sh
scripts/release.sh            # build + verify reproducibility
scripts/release.sh --sign     # also sign with your own cosign identity
```

`scripts/release.sh` rebuilds twice and compares the SHA256s. If
anything differs, the script exits non-zero so CI catches it.

## What "reproducible" means here

The release binary is compiled with:

* `SOURCE_DATE_EPOCH=1700000000` — a fixed timestamp for any embedded
  build-time constants.
* `RUSTFLAGS="-C link-arg=-Wl,--build-id=none"` — drop the build ID
  (it embeds the host path and timestamp otherwise).
* `RUSTFLAGS="-C debuginfo=0"` — no debug info that varies between
  hosts.

Toolchain is pinned via `rust-toolchain.toml` (currently `1.85.0`).
A change to the toolchain **will** change the SHA256, by design —
that change becomes the new pin.

## Audit log

Each release is also attached to a [Git tag][tags] signed by a
maintainer. Verify the tag locally with:

```sh
git verify-tag v0.1.0
```

[tags]: https://github.com/anomalyco/openaccounting/tags

---

## See also

- `openspec/changes/d4-reproducible-builds/specs/reproducible-builds/spec.md`
- `.github/workflows/release.yml`
- `scripts/release.sh`