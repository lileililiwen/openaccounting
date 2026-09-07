# Design — data-interchange

## Context

The reconciliation pipeline (`bank_statement_lines` → rules → match →
complete) is format-agnostic above the row shape. Importers only need
to produce that shape. Payee learning adds a feedback loop the
pipeline already lacks: confirmations currently vanish after `complete`.

## Goals / Non-Goals

**Goals:**
- One import UX regardless of file type.
- Matching improves with use, per ledger.

**Non-Goals:**
- Live aggregator onboarding (exists: bank-feeds).
- Cross-ledger alias sharing.
- HBCI/FinTS client (GnuCash/AqBanking territory; PSD2 aggregators
  already cover EU banks).

## Decisions

- **Hand-written parsers for OFX v1 SGML + QIF; `camt` handling via a
  small ISO 20022 subset deserializer using `quick-xml`** (already in
  tree via other deps — verify at apply time). Alternatives considered:
  `ofx-rs`/`rust-ofx` crates — unmaintained/thin; `ebics` crates —
  out of scope. MT940 is line-oriented text, parsed directly.
- **Learning signal = confirmed matches + manual edits only**, never
  raw imports. WHY: auto-matched-but-wrong rows would self-reinforce;
  GnuCash's matcher has the same discipline.
- **Confidence = hit_count × recency decay, threshold 3 effective
  hits for auto-fill.** WHY: one coincidence shouldn't move money;
  three consistent confirmations are the smallest honest signal.
- **Retroactive apply writes through the posting service and audit
  chain as one batch event.** WHY: closed-period guard and tamper-
  evident history stay authoritative.

## Risks / Trade-offs

- QIF ambiguity (no currency, ambiguous dates) → Mitigation: date-format
  detection with user override in wizard step 1; currency always from
  account.
- CAMT namespace drift across banks → Mitigation: parse by local-name,
  test corpus from ≥5 real bank files committed as fixtures.
- Alias poisoning via shared ledgers → Mitigation: aliases are
  ledger-scoped; editor role required to confirm matches.
- Parser memory on huge files → Mitigation: streaming parse, cap at
  10k lines per batch like CSV wizard.

## Migration Plan

1. Extract shared "statement line" normalizer from CSV path.
2. Add parsers behind content sniffing; wire into import route.
3. Alias table + capture points (confirm, edit).
4. Suggestion endpoints + retroactive apply action.
