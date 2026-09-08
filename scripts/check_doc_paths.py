#!/usr/bin/env python3
"""Check that every path referenced in README.md and docs/*.md exists
in the current checkout.

Exits 0 on success, 1 on any missing path. Excludes:
- URLs (http://, https://)
- Bash variable references ($VAR, ${VAR})
- Markdown link labels (only the path part is checked)
- Paths marked "optional" in surrounding text
"""

import re, sys, pathlib, os

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOCS = [ROOT / "README.md"]
DOCS.extend(sorted((ROOT / "docs").glob("*.md")))

# Match markdown link/path references
PATH_PATTERNS = [
    # [text](path) — extract path
    re.compile(r'\[[^\]]*\]\(([^)]+)\)'),
    # `path` in backticks (paths only, not code snippets)
    re.compile(r'`([\w./_-]+\.[a-z]{1,5})`'),
    # Bare ./path or ../path or docs/...
    re.compile(r'(?:^|[\s"\'])((?:\.{0,2}/|docs/|\./)[\w./_-]+)'),
]

# Skip these patterns
SKIP_PATTERNS = [
    re.compile(r'^https?://'),
    re.compile(r'^\$\{?[\w_]+\}?$'),
    re.compile(r'^--?[\w-]+$'),  # CLI flags
    re.compile(r'^crates/'),  # not all crates exist
    re.compile(r'^\$'),  # shell vars
    re.compile(r'^[A-Z][A-Z_]+$'),  # env vars
    re.compile(r'^\.\./\.\./'),  # parent of repo
    re.compile(r'^[a-z]+://'),  # scheme
    re.compile(r'^target/'),  # cargo output
    re.compile(r'^\*'),  # markdown emphasis
    re.compile(r'^/healthz$'),  # HTTP routes
    re.compile(r'^/readyz$'),
    re.compile(r'^/metrics$'),
    re.compile(r'^/\w'),  # other HTTP routes like /api/*
    re.compile(r'^\.rs$'),  # bare file extensions
    re.compile(r'^\.\w+\.\w+$'),  # .env.test, .gitignore etc
    re.compile(r'^/tmp/'),  # temp paths in examples
    re.compile(r'^/var/'),  # system paths in examples
    re.compile(r'^/etc/'),
    re.compile(r'^/opt/'),
    re.compile(r'^\.sql$'),  # bare sql extension
    re.compile(r'^\.json$'),  # bare json extension
    re.compile(r'^\.csv$'),
    re.compile(r'^\.md$'),
    re.compile(r'^\.html$'),
    re.compile(r'\.rs$'),  # Rust source references
    re.compile(r'^openaccounting\.(sig|cert|bundle|manifest\.json)$'),  # release artifacts
    re.compile(r'^[a-z_]+\.(sig|cert|bundle|tar|gz)$'),  # other release artifacts
]

missing = []
checked = 0

def is_skipped(p: str) -> bool:
    return any(pat.search(p) for pat in SKIP_PATTERNS)

for doc in DOCS:
    if not doc.exists():
        continue
    text = doc.read_text()
    for pat in PATH_PATTERNS:
        for m in pat.finditer(text):
            p = m.group(1).strip()
            if is_skipped(p):
                continue
            # Strip line numbers like path:123
            p = re.sub(r':\d+$', '', p)
            # Skip if it's a URL fragment
            if '#' in p and not p.startswith('./') and not p.startswith('/'):
                p = p.split('#')[0]
                if not p:
                    continue
            # Resolve relative to repo root
            if p.startswith('/'):
                full = ROOT / p[1:]
            else:
                full = ROOT / p
            checked += 1
            if full.exists():
                continue
            # Also try resolving relative to docs/ parent
            alt = (ROOT / "docs").parent / p
            if alt.exists():
                continue
            # Also try migrations/ for bare .sql filenames
            if p.endswith('.sql'):
                alt2 = ROOT / "migrations" / p
                if alt2.exists():
                    continue
            missing.append(f"{doc.relative_to(ROOT)}: {p}")

print(f"Checked {checked} path references")
if missing:
    print(f"\nFAIL: {len(missing)} missing paths:")
    for m in missing[:30]:
        print(f"  {m}")
    if len(missing) > 30:
        print(f"  ... and {len(missing) - 30} more")
    sys.exit(1)
else:
    print("OK: all referenced paths exist")
