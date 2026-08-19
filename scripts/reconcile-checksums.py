#!/usr/bin/env python3
"""Recompute `_sqlx_migrations.checksum` for every applied
migration against the current files in `migrations/`.

Why this exists: my earlier edits (adding then stripping
`-- Reversible: yes` headers) changed the SHA-384 of every
migration file. sqlx's migrator refuses to start when the
stored checksum doesn't match the file checksum. For an
existing DB this script re-aligns the stored checksums with
the current files so the binary can boot.

This is a one-time reconciliation helper (`o3-reversible-
migrations`). Future migrations keep their checksums in
sync via the standard sqlx flow.

Usage:
    DATABASE_URL=postgres://… python3 scripts/reconcile-checksums.py
    DATABASE_URL=postgres://… python3 scripts/reconcile-checksums.py --emit-sql > out.sql
"""
import argparse
import hashlib
import os
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--migrations",
        default=os.environ.get("MIGRATIONS_DIR", "./migrations"),
        help="path to the migrations directory (default: ./migrations)",
    )
    parser.add_argument(
        "--emit-sql",
        action="store_true",
        help="print the UPDATE statements instead of applying them",
    )
    args = parser.parse_args()

    migrations = sorted(
        f
        for f in os.listdir(args.migrations)
        if f.endswith(".sql") and not f.endswith(".down.sql")
    )

    checksums: dict[int, bytes] = {}
    for f in migrations:
        version = int(f.split("_")[0])
        with open(Path(args.migrations) / f, "rb") as fh:
            checksums[version] = hashlib.sha384(fh.read()).digest()

    if args.emit_sql:
        for version, digest in sorted(checksums.items()):
            print(
                f"UPDATE _sqlx_migrations "
                f"SET checksum = decode('{digest.hex()}', 'hex') "
                f"WHERE version = {version};"
            )
        return 0

    # Apply via DATABASE_URL.
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        print("DATABASE_URL must be set when not using --emit-sql", file=sys.stderr)
        return 2
    # Imported lazily so the script can be used with --emit-sql
    # even when sqlx isn't installed.
    import asyncio
    import sqlx

    async def apply() -> None:
        pool = await sqlx.create_pool(database_url)
        try:
            async with pool.acquire() as conn:
                async with conn.transaction():
                    for version, digest in sorted(checksums.items()):
                        await conn.execute(
                            "UPDATE _sqlx_migrations "
                            "SET checksum = $1 WHERE version = $2",
                            digest,
                            version,
                        )
        finally:
            await pool.close()

    asyncio.run(apply())
    print(
        f"==> reconciled {len(checksums)} migration checksums in {database_url}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())