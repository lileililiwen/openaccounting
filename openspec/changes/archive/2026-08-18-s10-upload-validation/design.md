# ## Context

There is no current limit. Axum's `DefaultBodyLimit::max(bytes)` is the
canonical way.

## Goals / Non-Goals

**Goals:**
- Cheap defense against disk-fill DoS.
- Avoid MIME spoofing.

**Non-Goals:**
- Per-user quota persistence (separate change).

## Decisions

- `infer` crate for sniffing.
- Sniffed MIME stored in the `documents` row.
- Body limit enforced at the layer, not the handler.

## Risks / Trade-offs

- Some valid files (e.g. PDFs with no header in the first 4 KB) may fail.
  Unlikely in practice.
- CSV is the special case; we accept on declaration for that.
