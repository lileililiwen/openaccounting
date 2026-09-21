## Context

Agents.md section 9 already defines the consistency rule and its verification commands (`rg TBD`, stale identity check, `scripts/check_doc_paths.py`). The rule is currently violated by TBD purposes. CI runs formatting, clippy, tests, and migration reversibility.

## Goals / Non-Goals

**Goals:**
- Any newcomer can answer "what is next, what is done, how do I contribute" in ten minutes.
- Docs, specs, and code agree by construction in CI.

**Non-Goals:**
- Choosing an open-source foundation or CLA (governance covers contribution process only).
- Rewriting history or version numbers (forward-fix changelog discipline only).

## Decisions

- **ROADMAP as a single curated file with Now/Next/Later plus v1.0 gates, not per-change roadmaps.** WHY: one prioritized list prevents 95 specs from reading as 95 promises; gates make v1.0 a decision instead of drift.
- **Fix TBDs in place as Modified Capabilities of this change.** WHY: purpose rewrites are doc-only deltas owned here rather than scattered across feature changes.
- **GOVERNANCE with maintainer merge rules, triage SLA (acknowledge 5 business days), and lazy-consensus for non-breaking changes.** WHY: matches small-maintainer reality without inventing a foundation process.
- **ERD generated from migrations via checked-in diagram plus source-of-truth SQL comments.** WHY: hand-drawn ERDs rot; generation from migrations keeps them current.
- **No new docs platform.** WHY: Markdown in `docs/` keeps single-binary repo self-contained.

## Risks / Trade-offs

- Roadmap promises attract scope pressure → Mitigation: explicit non-goals section plus gate criteria any addition must meet.
- ERD generation tooling adds CI weight → Mitigation: generate on migration change only, check in the artifact.
- Triage SLA missed by small team → Mitigation: SLA is acknowledge-only, not resolve; status board shows backlog honestly.
