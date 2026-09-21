# Release Verification

OpenAccounting ships signed, reproducible release binaries. Every
binary attached to a [GitHub Release][releases] is:

1. **Reproducible** — two consecutive builds of the same source commit
   produce a bit-identical binary (verified by CI before signing).
2. **Signed** — every binary is signed with [`cosign`][cosign] using
   keyless OIDC tied to this GitHub repository.
3. **Provenance-attached** — every release includes a
   `openaccounting.manifest.json` with the exact commit, tag, Rust
   toolchain, source timestamp, SHA256, and artifact list.

[releases]: https://github.com/lileililiwen/openaccounting/releases
[cosign]: https://docs.sigstore.dev/cosign/

---

## 1. Download

Download the artifacts attached to the release:

| File | Purpose |
| ---- | ------- |
| `openaccounting` | The release binary itself. |
| `openaccounting.bundle` | Signature bundle (contains `.sig` + `.cert`). |
| `openaccounting.sig` | Signature (already inside the bundle). |
| `openaccounting.cert` | Signing certificate (already in the bundle). |
| `openaccounting.manifest.json` | Provenance manifest (commit, tag, toolchain, SHA256). |

You can download all of them via the GitHub CLI:

```sh
gh release download v0.1.0
```

## 2. Verify the SHA256

The `openaccounting.manifest.json` in the release lists the expected
SHA256. Compare it to what you computed locally:

```sh
sha256sum openaccounting
```

The hash must match `manifest.sha256` exactly.

## 3. Verify the signature

```sh
cosign verify-blob \
  --bundle openaccounting.bundle \
  --certificate-identity-regexp 'https://github.com/lileililiwen/openaccounting/.github/workflows/release.yml@refs/tags/v0.1.0' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  openaccounting
```

Replace `v0.1.0` with the tag you downloaded. The `--certificate-identity-regexp`
must match `https://github.com/<owner>/<repo>/.github/workflows/release.yml@refs/tags/<TAG>`
where `<owner>/<repo>` is the canonical repository identity (see
`git remote -v`).

The output must end with:

```
Verified OK
```

If verification fails, **do not run the binary**. Open an issue and
reference the tag.

## 4. Verify the provenance manifest

```sh
cat openaccounting.manifest.json
```

Confirm:

- `commit` matches the tag's commit SHA (`git rev-parse v0.1.0`).
- `toolchain` matches the version pinned in `rust-toolchain.toml`.
- `sha256` matches the binary's SHA256 (step 2).
- `artifacts` lists every file attached to the release.

## 5. Verify the SBOM and its signature

```sh
cosign verify-blob \
  --bundle openaccounting-sbom.json.bundle \
  --certificate-identity-regexp 'https://github.com/lileililiwen/openaccounting/.github/workflows/release.yml@refs/tags/<TAG>' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  openaccounting-sbom.json
```

where `<TAG>` is the release tag. Then confirm the SBOM covers the
locked dependency set (CycloneDX `components` match `Cargo.lock`):

```sh
sha256sum -c openaccounting.sha256
```

The binary itself carries embedded audit data (`cargo auditable`
build): artifact-to-source linkage holds even without the SBOM file.

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

Toolchain is pinned via `rust-toolchain.toml` (currently `1.95.0`).
A change to the toolchain **will** change the SHA256, by design —
that change becomes the new pin.

## Audit log

Each release is also attached to a [Git tag][tags] signed by a
maintainer. Verify the tag locally with:

```sh
git verify-tag v0.1.0
```

[tags]: https://github.com/lileililiwen/openaccounting/tags

## Release prerequisites (CI gates)

A release can only be published when all of these pass:

1. **Tag validation** — tag matches `^v[0-9]+\.[0-9]+\.[0-9]+(-…)?$`
   and resolves to the checked-out commit.
2. **Formatting** — `cargo fmt -- --check`.
3. **Lint** — `cargo clippy --features test-support --all-targets -- -D warnings`.
4. **Tests** — `cargo test --features test-support`.
5. **Migration reversibility** — every `migrations/*.sql` has a
   `-- !DOWN` block or is marked `Reversible: no`.
6. **Dependency audit** — `cargo audit --deny warnings` passes.
7. **Reproducibility** — two consecutive builds produce identical SHA256.
8. **Signing** — `cosign sign-blob` with keyless OIDC succeeds.

If any gate fails, the workflow exits non-zero and no GitHub Release
or signed binary is published.

---

## See also

- `openspec/specs/reproducible-builds/spec.md`
- `openspec/specs/release-integrity/spec.md`
- `.github/workflows/release.yml`
- `scripts/release.sh`