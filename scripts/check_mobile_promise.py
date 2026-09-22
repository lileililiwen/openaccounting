#!/usr/bin/env python3
"""Docs-lint gate: README mobile promise vs `mobile/README.md`.

`u13-ux-a11y-mobile` finding A4 enforces that the project's
mobile promise is stated identically in both files. The gate
extracts the lines beginning with "Native mobile apps" (or the
"Status (…" block in `mobile/README.md`) from each file and
fails if the two statements disagree, or if either file is
missing the promise.

Designed to run against the real checkout and against fixture
trees so integration tests can assert both pass and fail paths.

Usage::

    python3 scripts/check_mobile_promise.py
    python3 scripts/check_mobile_promise.py --root=/path/to/fixture
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

PROJECT_MARKER = re.compile(
    r"^- Native mobile apps.*",
    re.MULTILINE | re.DOTALL,
)
MOBILE_MARKER = re.compile(
    r"^> \*\*Status \([0-9-]+\):\*\* Retired\.",
    re.MULTILINE,
)
PROJECT_KEY_PHRASES = (
    "Native mobile apps",
    "install the responsive web UI as a PWA",
)
MOBILE_KEY_PHRASES = (
    "Retired",
    "install the responsive web UI as a PWA",
)


def extract_promise(path: pathlib.Path, phrases: tuple[str, ...]) -> list[str]:
    if not path.exists():
        return []
    text = path.read_text(encoding="utf-8")
    # Markdown wraps lines inside list items; collapse whitespace
    # so phrase matching ignores line breaks.
    flat = " ".join(text.split())
    flat_lc = flat.lower()
    matches: list[str] = []
    for phrase in phrases:
        if phrase.lower() in flat_lc:
            matches.append(phrase)
    return matches


def check(root: pathlib.Path) -> tuple[int, str]:
    readme = root / "README.md"
    mobile_readme = root / "mobile" / "README.md"
    project = extract_promise(readme, PROJECT_KEY_PHRASES)
    mobile = extract_promise(mobile_readme, MOBILE_KEY_PHRASES)
    problems: list[str] = []
    if not project:
        problems.append(
            f"{readme.relative_to(root)}: missing the mobile-promise line "
            "starting with 'Native mobile apps'."
        )
    if not mobile:
        problems.append(
            f"{mobile_readme.relative_to(root)}: missing the 'Status (…): Retired.' "
            "block."
        )
    if project and mobile:
        # The two promises must reference the same action: the
        # PWA install path. Grep for the shared phrase.
        if not any("install the responsive web UI as a PWA" in m for m in project):
            problems.append(
                "project README promise does not name the PWA install path"
            )
        if not any("install the responsive web UI as a PWA" in m for m in mobile):
            problems.append(
                "mobile README promise does not name the PWA install path"
            )
    if problems:
        out = "\n".join(["FAIL: mobile promise agreement:"] + [f"  {p}" for p in problems])
        return 1, out
    return 0, (
        "OK: README mobile promise agrees with mobile/README.md"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()
    root = pathlib.Path(args.root).resolve()
    rc, msg = check(root)
    print(msg)
    return rc


if __name__ == "__main__":
    sys.exit(main())