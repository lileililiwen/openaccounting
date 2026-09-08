# Design

## Decisions

### Use one canonical repository identity

The release workflow, README, and verification guide SHALL use the repository identity from the configured remote or an explicitly documented canonical value. The Cosign certificate identity regexp must be generated from the workflow repository context, not copied from a former project.

### Treat tags as release inputs

Release jobs SHALL require a tag matching the documented version format and SHALL publish a manifest containing commit SHA, toolchain, binary checksum, signature bundle, and test status. A workflow-dispatch release must validate that its requested tag resolves to the checked-out commit.

### Pin the supply chain

Third-party actions will be pinned to commit SHAs with human-readable version comments. Dependency audit is part of release readiness, while routine CI may run it on a separate schedule.

### Risks and mitigations

- SHA-pinned actions require maintenance; mitigate with a documented update procedure and Dependabot/Renovate configuration if adopted.
- Strict tag checks can block emergency builds; mitigate with an explicit, logged manual override that still requires artifact verification.

## Verification

Run the release workflow on a test tag, verify the binary checksum and Cosign identity using the generated repository-specific command, and confirm the release manifest identifies the exact commit.
