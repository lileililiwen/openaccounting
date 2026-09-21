#!/usr/bin/env python3
"""Fail if stale repository identity strings appear in docs or workflows.

The canonical owner/name comes from the git remote; strings from a
former project identity must never be copied into new docs
(Agents.md section 9). Accepts an alternate root so tests can run
the check against fixtures.
"""

import argparse
import pathlib
import sys

DEFAULT_FORBIDDEN = ["anomalyco"]


def find_stale(root: pathlib.Path, forbidden: list[str]) -> list[str]:
    targets = []
    readme = root / "README.md"
    if readme.exists():
        targets.append(readme)
    targets.extend(sorted((root / "docs").glob("*.md")))
    targets.extend(sorted((root / ".github" / "workflows").glob("*.yml")))
    hits = []
    for target in targets:
        try:
            text = target.read_text()
        except OSError:
            continue
        for bad in forbidden:
            if bad in text:
                hits.append(f"{target.relative_to(root)}: {bad}")
    return hits


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=None,
                        help="Repository root (or fixture root in tests); "
                             "defaults to the checkout containing scripts/")
    parser.add_argument("--forbidden", action="append",
                        default=None,
                        help="Stale string to reject (repeatable)")
    args = parser.parse_args()

    script_parent = pathlib.Path(__file__).resolve().parent.parent
    if args.root is None:
        root = script_parent
    else:
        root = pathlib.Path(args.root)
        if not root.is_absolute():
            root = pathlib.Path.cwd() / root
    forbidden = args.forbidden or DEFAULT_FORBIDDEN

    hits = find_stale(root, forbidden)
    if hits:
        print(f"FAIL: {len(hits)} stale identity reference(s):")
        for hit in hits:
            print(f"  {hit}")
        return 1
    print(f"OK: no stale identity strings under {root}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
