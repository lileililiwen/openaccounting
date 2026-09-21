#!/usr/bin/env python3
"""Fail if CHANGELOG.md lacks an Unreleased section with migration notes.

Every user-facing change lands under Unreleased, and any release
carrying migrations must tell operators what to apply or revert, so
the Unreleased section must contain a migration-notes heading while
``migrations/`` holds schema files. Accepts alternate paths so tests
can run the check against fixtures.
"""

import argparse
import pathlib
import re
import sys

UNRELEASED = re.compile(r"^## \[Unreleased\]", re.IGNORECASE)
HEADING = re.compile(r"^##\s")
MIGRATION_NOTE = re.compile(r"^#+\s.*migrat", re.IGNORECASE)


def check(changelog: pathlib.Path, migrations_dir: pathlib.Path) -> list[str]:
    try:
        text = changelog.read_text()
    except OSError:
        return [f"changelog not found: {changelog}"]
    lines = text.splitlines()
    start = next((i for i, line in enumerate(lines) if UNRELEASED.match(line.strip())), None)
    if start is None:
        return ["CHANGELOG.md has no '## [Unreleased]' section"]
    section = []
    for line in lines[start + 1:]:
        if HEADING.match(line.strip()):
            break
        section.append(line)
    problems = []
    if not any(line.strip() for line in section):
        problems.append("'## [Unreleased]' section is empty")
    has_migrations = any(migrations_dir.glob("*.sql"))
    if has_migrations and not any(MIGRATION_NOTE.match(line.strip()) for line in section):
        problems.append("'## [Unreleased]' has no migration-notes heading")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--migrations-dir", default="migrations")
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    changelog = pathlib.Path(args.changelog)
    if not changelog.is_absolute():
        changelog = root / changelog
    migrations_dir = pathlib.Path(args.migrations_dir)
    if not migrations_dir.is_absolute():
        migrations_dir = root / migrations_dir

    problems = check(changelog, migrations_dir)
    if problems:
        print("FAIL: CHANGELOG discipline:")
        for problem in problems:
            print(f"  {problem}")
        return 1
    print(f"OK: {changelog} has Unreleased entries with migration notes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
