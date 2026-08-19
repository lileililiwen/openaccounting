# Reversible Migrations

## Why

All 38 existing migrations are forward-only. A failed v0.2 deploy
cannot roll back without hand-writing reverse SQL. sqlx supports
reversible migrations via the `--reversible` flag and `down.sql`
sections.

## What Changes

- Re-author each existing migration with a `-- Reversible: …` header
  and a DOWN block.
- New migrations MUST ship with both UP and DOWN.
- `sqlx migrate add --reversible <name>` becomes the canonical command.
- A CI step runs `sqlx migrate revert` against a fresh DB and re-applies
  to prove reversibility.

## Capabilities

### New Capabilities

- `reversible-migrations`: Bidirectional migrations.

## Impact

**Modified files:**
- All `migrations/00xx_*.sql` files get a DOWN block.
- `.github/workflows/ci.yml` — new step.
- `README.md` — migration policy.

**New files:**
- `scripts/migrate-down.sh`.
