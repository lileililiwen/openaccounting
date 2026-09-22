#!/usr/bin/env python3
"""WCAG audit gate (`u13-ux-a11y-mobile`).

Enforces two rules from `docs/wcag-audit-2026-09-21.md`:

1. No P1 finding id `A1..A4` may appear with `status: open` while
   the project README claims "WCAG 2.2 AA" or
   "accessibility conformance". The check inverts: an open
   P1 blocks the claim.
2. The locale coverage artifact (`docs/locale-coverage.json`)
   must exist and pass its threshold when the README claims
   locale support for the day-1 languages.

Usage::

    python3 scripts/check_a11y_audit.py
    python3 scripts/check_a11y_audit.py --audit=path/to/audit.md \
        --readme=path/to/README.md \
        --coverage=path/to/coverage.json
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

P1_OPEN_RE = re.compile(
    r"\|\s*(A[1-9])\s*\|[^|]*\|[^|]*\|[^\n]*\bstatus:\s*open\b",
    re.IGNORECASE,
)
CONFORMANCE_PHRASES = (
    "WCAG 2.2 AA",
    "accessibility conformance",
    "WCAG 2.2 conformance",
)
THRESHOLD_PCT = 5.0


def check(
    audit: pathlib.Path,
    readme: pathlib.Path,
    coverage: pathlib.Path,
) -> tuple[int, str]:
    problems: list[str] = []
    if not audit.exists():
        problems.append(f"audit report missing: {audit}")
        return 1, "\n".join(["FAIL: a11y audit gate:"] + [f"  {p}" for p in problems])
    audit_text = audit.read_text(encoding="utf-8")
    opens = sorted(set(m.group(1) for m in P1_OPEN_RE.finditer(audit_text)))
    claims_aa = any(
        phrase.lower() in readme.read_text(encoding="utf-8").lower()
        for phrase in CONFORMANCE_PHRASES
    ) if readme.exists() else False
    if opens and claims_aa:
        problems.append(
            f"README claims accessibility conformance while P1 findings remain open: "
            + ", ".join(opens)
        )
    if not coverage.exists():
        problems.append(f"locale coverage artifact missing: {coverage}")
    else:
        try:
            data = json.loads(coverage.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            problems.append(f"locale coverage artifact is not valid JSON: {exc}")
            data = None
        if data:
            for code, entry in (data.get("locales") or {}).items():
                if entry.get("missing_pct", 0.0) > THRESHOLD_PCT:
                    problems.append(
                        f"locale {code} is above the {THRESHOLD_PCT}% missing-key "
                        f"threshold: {entry.get('missing_pct')}%"
                    )
    if problems:
        return 1, "\n".join(["FAIL: a11y audit gate:"] + [f"  {p}" for p in problems])
    return 0, "OK: a11y audit gate (no open P1 while claiming conformance, coverage within threshold)"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    repo_root = pathlib.Path(__file__).resolve().parent.parent
    parser.add_argument("--audit", default=str(repo_root / "docs/wcag-audit-2026-09-21.md"))
    parser.add_argument("--readme", default=str(repo_root / "README.md"))
    parser.add_argument("--coverage", default=str(repo_root / "docs/locale-coverage.json"))
    args = parser.parse_args()
    rc, msg = check(
        pathlib.Path(args.audit),
        pathlib.Path(args.readme),
        pathlib.Path(args.coverage),
    )
    print(msg)
    return rc


if __name__ == "__main__":
    sys.exit(main())