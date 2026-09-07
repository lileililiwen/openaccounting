# scheduler-worker Specification (delta)

## ADDED Requirements

### Requirement: DB-Backed Job Queue

The system SHALL implement a `jobs` table
`(id, kind, payload JSONB, run_at, attempts, max_attempts, status,
last_error, created_at)` processed by a tokio worker spawned at
startup. Claiming a job SHALL use `FOR UPDATE SKIP LOCKED` so two
instances never run the same job. Failed jobs SHALL retry with
exponential backoff (attempts ≤ `max_attempts`, default 5) and end in
status `dead` with `last_error` preserved.

#### Scenario: Two instances, one job

- **WHEN** two server processes poll simultaneously and one due job exists
- **THEN** exactly one process claims it; the other sees zero due jobs.

#### Scenario: Poison job dies after retries

- **WHEN** a job handler errors on every attempt up to `max_attempts`
- **THEN** the row ends `status='dead'` with the last error text and no
  further runs are scheduled.

### Requirement: Recurring Templates Post Automatically

The scheduler SHALL enqueue template-run jobs daily; each job posts
every due recurring template occurrence exactly once (idempotent by
`(template_id, due_date)`). The manual `POST /templates/process_due`
route SHALL remain and share the same idempotency key.

#### Scenario: Monthly rent posts without human action

- **WHEN** a monthly template is due on the 1st and the scheduler runs
  on the 1st and again on the 2nd
- **THEN** exactly one transaction per due date exists.

### Requirement: Scheduled Invoice Reminders

For each unpaid invoice past its due date, the scheduler SHALL emit an
`invoice.overdue` notification event on day 1, 7, and 14 past due,
configurable per ledger (default on). Reminders MUST NOT repeat for
the same `(invoice_id, offset_day)`.

#### Scenario: Overdue reminder fires once per offset

- **WHEN** an invoice is 8 days overdue and the scheduler has run daily
- **THEN** reminders exist for offsets 1 and 7 only.
