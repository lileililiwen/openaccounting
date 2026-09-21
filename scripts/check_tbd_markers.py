#!/usr/bin/env python3
"""Fail if any accepted capability spec still carries a TBD placeholder.

Scans ``openspec/specs/**/spec.md`` for the archiving placeholder
``TBD - created by archiving``. Accepts an alternate specs directory
so tests can run the check against fixtures.
"""

import argparse
import pathlib
import sys

MARKER = "TBD - created by archiving"


def purpose_section(text: str) -> str:
    """Return the `## Purpose` section body (empty if absent).

    Only a placeholder Purpose is a violation; requirements may
    legitimately *name* the marker when they ban it.
    """
    lines = text.splitlines()
    start = next(
        (i for i, line in enumerate(lines) if line.strip() == "## Purpose"),
        None,
    )
    if start is None:
        return ""
    body = []
    for line in lines[start + 1:]:
        if line.startswith("## "):
            break
        body.append(line)
    return "\n".join(body)


def find_tbd(specs_dir: pathlib.Path) -> list[str]:
    hits = []
    for spec in sorted(specs_dir.glob("**/spec.md")):
        try:
            text = spec.read_text()
        except OSError:
            continue
        if MARKER in purpose_section(text):
            hits.append(str(spec))
    return hits


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--specs-dir",
        default="openspec/specs",
        help="Directory holding accepted capability specs",
    )
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    specs_dir = pathlib.Path(args.specs_dir)
    if not specs_dir.is_absolute():
        specs_dir = root / specs_dir

    hits = find_tbd(specs_dir)
    if hits:
        print(f"FAIL: {len(hits)} spec(s) still carry a TBD purpose:")
        for hit in hits:
            print(f"  {hit}")
        return 1
    print(f"OK: no TBD markers under {specs_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
