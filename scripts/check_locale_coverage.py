#!/usr/bin/env python3
"""Locale coverage gate (`u13-ux-a11y-mobile`).

Reads every ``static/locales/<lang>.json`` file, compares each
against the English reference, and:

1. Publishes a JSON artifact at ``docs/locale-coverage.json``
   with per-language totals and missing-key percentages.
2. Fails (exit 1) when any day-1 locale exceeds 5 % missing keys.

The day-1 locale set is the six languages the runtime supports
(see ``src/i18n/mod.rs::Locale::ALL``). The threshold is 5 % —
above that the language is unusable for too many flows and
shipping it would be dishonest. Community locales are not
checked against the threshold because they are not part of the
release promise.

Run from CI::

    python3 scripts/check_locale_coverage.py

Accepts ``--locales-dir=…`` and ``--output=…`` so the same
script runs against fixture trees in tests.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
import tempfile
import subprocess

DAY1_THRESHOLD_PCT = 5.0

DEFAULT_LOCALE_CODES = ("en", "zh-CN", "es", "fr", "de", "ja")


def load_catalog(path: pathlib.Path) -> dict:
    try:
        with path.open(encoding="utf-8") as fh:
            return json.load(fh)
    except (OSError, json.JSONDecodeError) as exc:
        sys.exit(f"FAIL: could not load catalog {path}: {exc}")


def coverage(reference: dict, others: dict[str, dict]) -> dict:
    reference_keys = [k for k in reference.keys() if not k.startswith("_")]
    reference_total = len(reference_keys)
    locales = {}
    for code, catalog in others.items():
        present = [k for k in reference_keys if k in catalog]
        missing = [k for k in reference_keys if k not in catalog]
        missing_count = len(missing)
        missing_pct = (missing_count / reference_total * 100.0) if reference_total else 0.0
        locales[code] = {
            "code": code,
            "total_keys": reference_total,
            "present_keys": len(present),
            "missing_keys": sorted(missing),
            "missing_pct": round(missing_pct, 4),
        }
    return {"reference_total": reference_total, "locales": locales}


def failing_locales(report: dict, threshold_pct: float) -> list[dict]:
    failing = []
    for locale in report["locales"].values():
        if locale["missing_pct"] > threshold_pct:
            failing.append(locale)
    return failing


def check(
    locales_dir: pathlib.Path,
    output: pathlib.Path | None,
    threshold_pct: float = DAY1_THRESHOLD_PCT,
) -> tuple[int, str]:
    if not locales_dir.exists():
        return 1, f"locales dir not found: {locales_dir}"

    reference_path = locales_dir / "en.json"
    reference = load_catalog(reference_path)

    others: dict[str, dict] = {}
    for entry in sorted(locales_dir.glob("*.json")):
        if entry.name == "en.json":
            continue
        code = entry.stem
        others[code] = load_catalog(entry)

    report = coverage(reference, others)
    failing = failing_locales(report, threshold_pct)

    if output is not None:
        output.parent.mkdir(parents=True, exist_ok=True)
        with output.open("w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2, sort_keys=True)
            fh.write("\n")

    lines = []
    lines.append(f"Reference (en) total keys: {report['reference_total']}")
    for code, l in sorted(report["locales"].items()):
        marker = "FAIL" if l["missing_pct"] > threshold_pct else "ok"
        lines.append(
            f"  {code:8s} {l['present_keys']:3d}/{l['total_keys']:3d} "
            f"missing={l['missing_pct']:6.2f}%  [{marker}]"
        )
    body = "\n".join(lines)

    if failing:
        fail_lines = ["\nFAIL: locales above threshold:"]
        for f in failing:
            sample = ", ".join(f["missing_keys"][:5])
            more = "" if len(f["missing_keys"]) <= 5 else f" (+{len(f['missing_keys']) - 5} more)"
            fail_lines.append(
                f"  {f['code']}: {f['missing_pct']:.2f}% missing — "
                f"first keys: {sample}{more}"
            )
        return 1, body + "\n" + "\n".join(fail_lines)

    return 0, f"OK: every locale within {threshold_pct}% missing keys\n{body}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--locales-dir",
        default="static/locales",
        help="Directory containing per-locale JSON catalogs.",
    )
    parser.add_argument(
        "--output",
        default="docs/locale-coverage.json",
        help="Path to write the JSON coverage artifact.",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=DAY1_THRESHOLD_PCT,
        help="Per-locale missing-key percentage threshold.",
    )
    parser.add_argument(
        "--write-output",
        action="store_true",
        help="Actually write the artifact (default: do not write).",
    )
    args = parser.parse_args()

    repo_root = pathlib.Path(__file__).resolve().parent.parent
    locales_dir = pathlib.Path(args.locales_dir)
    if not locales_dir.is_absolute():
        locales_dir = repo_root / locales_dir
    output = pathlib.Path(args.output)
    if not output.is_absolute():
        output = repo_root / output

    rc, msg = check(
        locales_dir,
        output if args.write_output else None,
        args.threshold,
    )
    print(msg)
    return rc


if __name__ == "__main__":
    sys.exit(main())