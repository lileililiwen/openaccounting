# Governance

How OpenAccounting is maintained, how contributions land, and how fast
maintainers respond. This covers process only; no foundation or CLA
is required to contribute.

## Maintainers

Listed in `CODEOWNERS`. Maintainers merge, tag releases, and steward
the roadmap. Decisions are recorded in OpenSpec proposals or in the
merged PR discussion.

## Merge rules

- Every non-trivial change goes through OpenSpec first (see
  `Agents.md` §2): propose, validate, implement, archive.
- `cargo fmt --check`, clippy with `-D warnings`, and the full test
  suite must pass; the docs-consistency checks
  (`scripts/check_tbd_markers.py`, `scripts/check_identity.py`,
  `scripts/check_doc_paths.py`, `scripts/check_roadmap_entries.py`,
  `python3 scripts/generate_erd.py --check`) must pass.
- Migrations must be reversible or explicitly marked irreversible
  with a documented reason.
- At least one maintainer review is required before merge to `main`.

## Triage SLA

[![Triage SLA](https://img.shields.io/badge/triage-ack%20in%205%20business%20days-blue)](GOVERNANCE.md#triage-sla)

Maintainers acknowledge new issues and pull requests within **5
business days**. Acknowledgement means a label plus a short reply
(accepted, needs-info, or declined with a reason) — not a fix or a
merge. There is no promised resolution time; the issue board shows
the backlog honestly.

## Decision process

- Non-breaking changes: lazy consensus — a maintainer may merge
  after review with no sustained objection within 5 business days.
- Breaking changes (migrations that alter semantics, API shape
  changes, auth changes): require an OpenSpec change plus explicit
  maintainer approval.
- Roadmap order changes: proposed as a `ROADMAP.md` edit with a
  written reason; the `current_spec:` pointer in `HANDOFF.md`
  advances only at a change-cycle boundary.

## Security reports

See `SECURITY.md`. Do not file public issues for vulnerabilities.
