#!/usr/bin/env python3
"""Fail if SECURITY.md names a placeholder security contact.

An operative contact (a real address, no INSERT/TODO markers) is
required so a disclosed vulnerability reaches a human. Accepts an
alternate path so tests can run the check against fixtures.
"""

import argparse
import pathlib
import re
import sys

PLACEHOLDERS = [
    re.compile(r"\[INSERT[^\]]*\]", re.IGNORECASE),
    re.compile(r"\[TODO[^\]]*\]", re.IGNORECASE),
    re.compile(r"TODO:\s*\S*@example\.|security@example\.", re.IGNORECASE),
    re.compile(r"you@example\.|your-email@", re.IGNORECASE),
]

EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")


def check(path: pathlib.Path) -> list[str]:
    try:
        text = path.read_text()
    except OSError:
        return [f"SECURITY.md not found: {path}"]
    problems = []
    for pattern in PLACEHOLDERS:
        if pattern.search(text):
            problems.append(f"placeholder contact marker: {pattern.pattern}")
    if not EMAIL.search(text):
        problems.append("no operative contact email address found")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--security-md", default="SECURITY.md")
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    path = pathlib.Path(args.security_md)
    if not path.is_absolute():
        path = root / path

    problems = check(path)
    if problems:
        print("FAIL: SECURITY.md contact:")
        for problem in problems:
            print(f"  {problem}")
        return 1
    print(f"OK: {path} names an operative security contact")
    return 0


if __name__ == "__main__":
    sys.exit(main())
