# Sizing guide

Rough capacity planning for a self-hosted instance. These are
starting points, not guarantees — measure with `/metrics` and size
from observed usage.

## Small (personal / family)

- Ledgers: 1–3, transactions: up to ~10k/year, users: 1–5.
- 1 vCPU, 1 GB RAM for the app plus managed or local PostgreSQL.
- `DOCUMENTS_DIR` on any persistent volume; tens of GB is ample.

## Medium (small business)

- Ledgers: up to ~20, transactions: up to ~200k/year, users: up to ~30.
- 2 vCPU, 2–4 GB RAM for the app; PostgreSQL 16 with 2 vCPU / 4 GB.
- Put PostgreSQL on SSD storage and keep automated backups
  (`docs/backup-restore.md`).

## Notes

- The app is a single binary; scale vertically first. Run one
  instance per database.
- Upload throughput is bounded by `UPLOAD_MAX_BYTES` (default 25 MiB)
  and `DOCUMENTS_DIR` disk, not by CPU.
- Report pages aggregate in SQL; very large ledgers (millions of
  postings) benefit from more database memory before more app CPU.
- The data-model shape behind these numbers is in `docs/erd.md`.
