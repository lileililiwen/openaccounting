# ## Context

Without export, data is hostage.

## Goals / Non-Goals

**Goals:**
- Two formats. Round-trippable.

**Non-Goals:**
- OFX / QIF / MT940 (banks).

## Decisions

- JSON is canonical; Beancount is derived.
- Both produced under one REPEATABLE READ snapshot.

## Risks / Trade-offs

- Streaming the JSON via `axum::body::StreamBody` and `tokio::io::AsyncWrite`.
