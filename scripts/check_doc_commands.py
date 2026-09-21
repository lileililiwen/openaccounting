#!/usr/bin/env python3
"""Fail if a fenced shell command in the PITR doc is untestable.

Every ```bash/sh block in the checked docs must start each command
with a known binary (installed alongside the app or database) or
reference an existing `scripts/` entrypoint, so a user following
the procedure never hits a missing command. Accepts alternate paths
so tests can run the check against fixtures.
"""

import argparse
import pathlib
import re
import shlex
import sys

DEFAULT_DOCS = ["docs/backup-restore.md"]

# Binaries the deployment environment provides (app toolchain,
# postgres client kit, coreutils) plus shell builtins used for
# output capture in examples.
KNOWN_BINARIES = {
    "pg_dump", "pg_restore", "psql", "pg_basebackup", "createdb",
    "dropdb", "tar", "gzip", "gunzip", "openssl", "sha256sum",
    "curl", "cargo", "sqlx", "docker", "systemctl", "sudo",
    "mkdir", "cp", "mv", "rm", "ls", "cat", "head", "echo",
    "export", "cd", "sqlite3", "openaccounting", "touch", "chmod",
}

FENCE = re.compile(r"^```(\w*)\s*$")


def check_doc(doc: pathlib.Path, root: pathlib.Path) -> list[str]:
    try:
        text = doc.read_text()
    except OSError:
        return [f"doc not found: {doc}"]
    problems = []
    in_block, lang = False, ""
    continuation, in_quotes, heredoc = False, False, None
    for lineno, line in enumerate(text.splitlines(), start=1):
        fence = FENCE.match(line.strip())
        if fence:
            if in_block:
                in_block = False
            else:
                in_block, lang = True, fence.group(1)
            continuation, in_quotes, heredoc = False, False, None
            continue
        if not in_block or lang not in ("bash", "sh", "console"):
            continue
        # Heredoc bodies (<<EOF ... EOF) are data, not commands.
        if heredoc is not None:
            if line.strip() == heredoc:
                heredoc = None
            continue
        heredoc_match = re.search(r"<<-?\s*'?\"?(\w+)'?\"?", line)
        if heredoc_match:
            heredoc = heredoc_match.group(1)
        # A trailing backslash continues the command on the next
        # line; multi-line double-quoted strings (psql -c "...")
        # are data, not commands.
        this_continuation = line.rstrip().endswith("\\")
        if continuation or in_quotes:
            if line.count('"') % 2 == 1:
                in_quotes = not in_quotes
            continuation = this_continuation
            continue
        if line.count('"') % 2 == 1:
            in_quotes = True
            # A quoted string may still start with a real command
            # (psql ... -c "); fall through and check it.
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continuation = this_continuation
            continue
        try:
            parts = shlex.split(stripped, posix=True)
        except ValueError:
            continuation = this_continuation
            continue
        # Skip VAR=env prefixes: the command is the first token
        # without an unquoted `=`.
        while parts and "=" in parts[0] and not parts[0].startswith("-"):
            parts = parts[1:]
        if not parts:
            continuation = this_continuation
            continue
        cmd = parts[0].lstrip("$").rstrip("\\")
        continuation = this_continuation
        if cmd.startswith("-"):
            continue
        if cmd in ("|", "&&", "||", ";", "!", "{", "}", "if", "then",
                   "else", "fi", "for", "do", "done", "while", "set"):
            continue
        if cmd in KNOWN_BINARIES:
            continue
        if cmd.startswith("scripts/"):
            if not (root / cmd.split()[0]).exists():
                problems.append(f"{doc}:{lineno}: missing script {cmd}")
            continue
        if "/" in cmd and (root / cmd).exists():
            continue
        problems.append(f"{doc}:{lineno}: unknown command `{cmd}`")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--docs", action="append", default=None)
    parser.add_argument("--root", default=None)
    args = parser.parse_args()

    script_parent = pathlib.Path(__file__).resolve().parent.parent
    root = pathlib.Path(args.root) if args.root else script_parent
    docs = args.docs or DEFAULT_DOCS

    problems = []
    for doc in docs:
        path = pathlib.Path(doc)
        if not path.is_absolute():
            path = root / path
        problems.extend(check_doc(path, root))
    if problems:
        print("FAIL: untestable doc commands:")
        for problem in problems:
            print(f"  {problem}")
        return 1
    print("OK: doc shell commands resolve to known binaries or scripts")
    return 0


if __name__ == "__main__":
    sys.exit(main())
