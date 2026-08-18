# Health and Readiness Endpoints

## Why

The auth spec mentions `/healthz` as a "future change" (`auth/spec.md:101`).
Today there is nothing for a load balancer or Kubernetes to probe. The
server may be up but unable to reach Postgres, which an LB cannot
distinguish from healthy.

## What Changes

- `GET /healthz` returns 200 if the process is alive (liveness probe).
- `GET /readyz` returns 200 if Postgres is reachable AND the documents
  directory is writable; otherwise 503 (readiness probe).
- Both endpoints are public (no auth).

## Capabilities

### New Capabilities

- `health-endpoints`: /healthz and /readyz probes.

## Impact

**New files:**
- `src/handlers/health.rs`.
- `tests/http/health.rs`.
