# metrics Specification

## Purpose
TBD - created by archiving change o4-metrics-endpoint. Update Purpose after archive.
## Requirements
### Requirement: Metrics Endpoint

MUST expose `GET /metrics` returning Prometheus exposition format; the endpoint MUST be unauthenticated and rate-limit-exempt.

#### Scenario: Scrape

- **WHEN** a Prometheus server hits /metrics
- **THEN** the response is 200 with the exposition format.

### Requirement: HTTP Metrics

MUST publish counters for requests by route, method, status; histograms for request latency.

#### Scenario: Counter increments

- **WHEN** the user POSTs /transactions
- **THEN** the counter for that route + status is incremented.

### Requirement: Domain Metrics

MUST publish counters for postings created, reconciliations completed, backups completed, failed-login attempts.

#### Scenario: Counter

- **WHEN** a transaction is created
- **THEN** `postings_created_total` increments.

### Requirement: No Secret Leakage

MUST NOT include any user-identifying data, passwords, or transaction content; only counts and timing.

#### Scenario: Privacy

- **WHEN** a request includes a session cookie
- **THEN** the metrics response does not contain the cookie.

### Requirement: Gating

MUST allow disabling via `METRICS_ENABLED=false` for dev/test runs.

#### Scenario: Disabled

- **WHEN** METRICS_ENABLED=false
- **THEN** the /metrics route is not registered.

