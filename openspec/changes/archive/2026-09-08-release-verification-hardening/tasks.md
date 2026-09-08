# Tasks

## 1. Testing

- [ ] 1.1 Add a workflow check that fails on stale repository identity strings and unpinned release actions.
- [ ] 1.2 Add a tag/commit mismatch test for workflow-dispatch inputs.
- [ ] 1.3 Define a test-tag verification procedure and record expected checksum/signature assertions.

## 2. Implementation

- [ ] 2.1 Replace hard-coded repository identities in README, release docs, and workflow-generated notes.
- [ ] 2.2 Add tag format and tag-to-commit validation to the release workflow.
- [ ] 2.3 Publish a provenance manifest with commit, tag, toolchain, checksum, and artifact names.
- [ ] 2.4 Pin external workflow actions and document the update policy.
- [ ] 2.5 Add dependency-audit status to the release prerequisites.

## 3. Verification

- [ ] 3.1 Run all CI quality gates on the release candidate.
- [ ] 3.2 Run the reproducible build twice and compare hashes.
- [ ] 3.3 Verify the signed artifact with the repository-specific Cosign identity.
- [ ] 3.4 Confirm a failing prerequisite cannot publish a release.
