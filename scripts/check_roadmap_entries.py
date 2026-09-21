#!/usr/bin/env python3
"""Fail if an active OpenSpec change is missing from ROADMAP.md.

Every directory directly under ``openspec/changes/`` (except
``archive/``) is an active package and must be named in the roadmap
so newcomers can answer "what is next". Accepts alternate paths so
tests can run the check against fixtures.
"""

import argparse
import pathlib
import sys


def missing_entries(changes_dir: pathlib.Path, roadmap: pathlib.Path) -> list[str]:
    try:
        text = roadmap.read_text()
    except OSError:
        return [f"roadmap not found: {roadmap}"]
    missing = []
    for child in sorted(changes_dir.iterdir()):
        if not child.is_dir() or child.name == "archive":
            continue
        if child.name not in text:
            missing.append(child.name)
    return missing


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--changes-dir", default="openspec/changes")
    parser.add_argument("--roadmap", default="ROADMAP.md")
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    changes_dir = pathlib.Path(args.changes_dir)
    if not changes_dir.is_absolute():
        changes_dir = root / changes_dir
    roadmap = pathlib.Path(args.roadmap)
    if not roadmap.is_absolute():
        roadmap = root / roadmap

    missing = missing_entries(changes_dir, roadmap)
    if missing:
        print(f"FAIL: {len(missing)} active change(s) missing from {roadmap}:")
        for name in missing:
            print(f"  {name}")
        return 1
    print(f"OK: all active changes are listed in {roadmap}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
