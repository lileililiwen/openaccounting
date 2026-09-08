# Release verification hardening

## Why

The repository has a promising reproducible-build and Cosign workflow, but its verification examples reference a different GitHub repository than the configured origin. There are no tags in the current checkout, and the workflow uses mutable action tags. A user following the documented command can therefore reject a valid release or verify against the wrong identity.

## What changes

- Add capability `release-integrity`.
- Make release identity and verification commands derive from the actual repository.
- Define a tagged-release checklist including build, test, checksum, signature, artifact, and rollback evidence.
- Pin third-party GitHub Actions to reviewed immutable references or establish an equivalent controlled policy.
- Add a dry-run or staging verification path before publishing a release.

## Non-goals

- No change to the application runtime or package format.
- No replacement of Cosign/Sigstore.
- No automatic production deployment.
