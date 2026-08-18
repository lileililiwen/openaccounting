# ## Context

Zero observability today beyond logs.

## Goals / Non-Goals

**Goals:**
- Standard Prometheus surface.

**Non-Goals:**
- Distributed tracing (separate change).
- OpenTelemetry export (could be added later as another exporter).

## Decisions

- `metrics-exporter-prometheus` is the canonical Rust crate.
- Histogram buckets cover 1 ms – 10 s (HTTP).
- Domain metrics are emitted from inside the handlers (not via a global
  recorder); simpler for v1.
