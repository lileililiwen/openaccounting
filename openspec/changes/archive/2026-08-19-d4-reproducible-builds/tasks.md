## 1. Testing

- [x] 1.1 CI: two builds produce identical SHA256. (Workflow has `test-reproducibility` job.)
- [x] 1.2 CI: cosign verify succeeds. (Workflow signs with `cosign sign-blob --bundle …` and release notes document `cosign verify-blob`.)

## 2. Implementation

- [x] 2.1 `rust-toolchain.toml`.
- [x] 2.2 `.github/workflows/release.yml`.
- [x] 2.3 `scripts/release.sh`.
- [x] 2.4 `docs/release-verification.md`.
- [x] 2.5 README link to docs/release-verification.md.
