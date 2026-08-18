# health-endpoints Specification

## Purpose
TBD - created by archiving change o5-health-endpoint. Update Purpose after archive.
## Requirements
### Requirement: Liveness

MUST respond to `GET /healthz` with HTTP 200 and the JSON `{ "status": "ok" }` whenever the process is alive.

#### Scenario: Liveness

- **WHEN** the process is running
- **THEN** 200 with `{"status":"ok"}`.

### Requirement: Readiness

MUST respond to `GET /readyz` with HTTP 200 only when Postgres is reachable AND the documents directory is writable; otherwise 503 with a `reason`.

#### Scenario: DB down

- **WHEN** Postgres is unreachable
- **THEN** 503 with `{"status":"not_ready","reason":"db"}`.

#### Scenario: Disk full

- **WHEN** the documents directory is not writable
- **THEN** 503 with reason `disk`.

#### Scenario: All good

- **WHEN** everything reachable
- **THEN** 200.

### Requirement: Public

MUST NOT require auth.

#### Scenario: Public

- **WHEN** an anonymous client hits /healthz
- **THEN** 200.

### Requirement: Cheap

MUST NOT perform expensive queries; readiness checks each subsystem with a 1 s timeout.

#### Scenario: Timeout

- **WHEN** the DB hangs
- **THEN** /readyz returns 503 within 1 s.

