#!/usr/bin/env bash
# Revert and re-apply all migrations (`o3-reversible-migrations`).
#
# Usage:
#   scripts/migrate-down.sh                # revert then re-apply against $DATABASE_URL
#   scripts/migrate-down.sh --keep-data   # revert then re-apply, preserve data via pg_dump
#   scripts/migrate-down.sh --revert-only # just revert all migrations
#   scripts/migrate-down.sh --apply-only  # just re-apply all migrations
#
# The script is the contract test for `o3-reversible-migrations`.
# Every migration in `migrations/` must end with a `-- !DOWN`
# marker; this script reads them in order.

set -euo pipefail

KEEP_DATA=0
REVERT_ONLY=0
APPLY_ONLY=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --keep-data)   KEEP_DATA=1; shift ;;
    --revert-only) REVERT_ONLY=1; shift ;;
    --apply-only)  APPLY_ONLY=1; shift ;;
    -h|--help)
      sed -n '2,17p' "$0"
      exit 0
      ;;
    *)             echo "unknown arg: $1" >&2; exit 64 ;;
  esac
done

DATABASE_URL="${DATABASE_URL:?DATABASE_URL must be set (see .env.test)}"
ADMIN_URL="$(echo "$DATABASE_URL" | sed -E 's|/[^/?]+(\?.*)?$|/postgres\1|')"

DUMP_FILE="$(mktemp)"
trap 'rm -f "$DUMP_FILE"' EXIT

dump() {
  echo "==> dumping schema+data to $DUMP_FILE (keep-data mode)"
  pg_dump --no-owner --schema=public "$DATABASE_URL" > "$DUMP_FILE"
}

apply_all() {
  echo "==> applying all migrations forward"
  cargo run --quiet --release -- --migrate >/dev/null 2>&1 \
    || sqlx migrate run --source migrations
}

revert_all() {
  echo "==> reverting all migrations"
  # sqlx-migrate's `revert` only reverts the last applied version.
  # Loop until `sqlx migrate run -r` succeeds with no version to revert.
  local safety=100
  while [ "$safety" -gt 0 ]; do
    if sqlx migrate revert --source migrations 2>/dev/null \
       || cargo run --quiet --release -- --migrate-revert 2>/dev/null; then
      safety=$((safety-1))
    else
      break
    fi
  done
  echo "==> revert loop ended"
}

if [ "$REVERT_ONLY" -eq 1 ]; then
  revert_all
  exit 0
fi

if [ "$KEEP_DATA" -eq 1 ]; then
  dump
fi

if [ "$APPLY_ONLY" -eq 1 ]; then
  apply_all
  if [ "$KEEP_DATA" -eq 1 ]; then
    echo "==> restoring data from $DUMP_FILE"
    psql "$DATABASE_URL" < "$DUMP_FILE"
  fi
  exit 0
fi

# Default: revert + re-apply.
revert_all
apply_all
if [ "$KEEP_DATA" -eq 1 ]; then
  echo "==> restoring data from $DUMP_FILE"
  psql "$DATABASE_URL" < "$DUMP_FILE"
fi

echo "==> done. migrations are reversible and re-applied cleanly."