# Agents.md

> This document is the contract for AI agents (and humans) working on the
> **openaccounting** codebase. It is **normative**: every principle here
> MUST be followed unless explicitly overridden by a written decision in
> an OpenSpec change.

---

## 1. What is openaccounting?

OpenAccounting is an open-source, Rust-based, **double-entry bookkeeping**
system for individuals and startups. It is a single static binary that
ships a responsive web UI (HTMX + Tailwind, server-rendered), a chart of
accounts, balanced transactions, document attachments, financial reports,
and server-rendered SVG visualizations. **No JavaScript build step is
required to run it.** MIT licensed.

**Built upon [vito](https://github.com/9-8-7-6/vito)** by 9-8-7-6 (MIT).
OpenAccounting retains vito's project skeleton (Axum layout, sqlx,
axum-login, Argon2, docker-compose) and rewrites the domain for proper
double-entry bookkeeping with reports, document upload, and
visualizations — see `NOTICE` and `README.md` for credit.

**Status:** v0.1-alpha. Core engine, auth, documents, reports, and
responsive web UI are tracked in the bootstrap change.

---

## 2. Spec-First Development (THE standing rule)

**Every change goes through OpenSpec before code is written.** This is
non-negotiable. It makes work trackable, reviewable, and resumable by
any other AI agent at any time.

```
propose  →  validate  →  implement (apply)  →  archive  →  spec is source of truth
```

### 2.1 OpenSpec workflow

| Phase | What happens | Output |
|---|---|---|
| `propose`   | Create `openspec/changes/<name>/` with `proposal.md`, `specs/<cap>/spec.md` (ADDED Requirements), `design.md`, `tasks.md` | A change folder |
| `validate`  | `openspec validate <name>` passes | Green |
| `apply`     | Implement tasks in order; check boxes in `tasks.md` | Working code |
| `archive`   | `openspec archive <name>` | Delta folded into `openspec/specs/<cap>/spec.md`; change moved to `archive/` |

### 2.2 Spec lifecycle

```
openspec/changes/<name>/
    ├── proposal.md                  ← why + what changes
    ├── specs/<cap>/spec.md          ← delta (ADDED Requirements)
    ├── design.md                    ← technical design (SQL, Rust, types)
    └── tasks.md                     ← numbered, checkable
       ↓ (on archive)
openspec/changes/archive/<date>-<name>/
    └── (frozen copy of the change)

openspec/specs/<cap>/spec.md         ← source of truth (post-archive)
```

**Source of truth** for any capability lives at
`openspec/specs/<cap>/spec.md`. Code that drifts from this is a bug.

### 2.3 Resumability — why we do this

When you (a future agent) start a new session on this project:

1. **Read** `openspec/changes/` first. There may be an in-flight change.
2. Open its `proposal.md`, `specs/<cap>/spec.md`, `design.md`, `tasks.md`.
3. Continue from the first unchecked box in `tasks.md`.
4. After `openspec validate` passes and tests pass, run `openspec archive`.
5. Pick up the next change (or create one for new work).

You do **not** need the original conversation to resume the work. The
specs are the contract.

### 2.4 Standing rule — tests come first in every change

> **Tests are written before code.** They exist to verify correctness,
> not to accommodate the code so it passes checks.

Every `tasks.md` for a code-related change MUST start with `## 1.
Testing`. That group MUST list at minimum:

- One unit test per new public function
- One integration test per new HTTP route
- One property test per new domain invariant
- One E2E test per new user-facing flow

The testing group is implemented **before** `## 2. Implementation` tasks
are marked complete. Tests MUST be specific: exact inputs, exact
assertions, exact error conditions.

---

## 3. Architecture

```
        ┌──────────────────────────────────────────────────────────┐
        │  Browser (HTMX + Tailwind, zero JS build)                │
        └────────────────────────┬─────────────────────────────────┘
                                 │ HTTP, server-rendered HTML
        ┌────────────────────────▼─────────────────────────────────┐
        │  openaccounting binary  (Axum 0.8 + tower)               │
        │   AuthManagerLayer  (axum-login)                          │
        │   SessionManagerLayer (Postgres-backed tower-sessions)    │
        │   tower-http TraceLayer, ServeDir for /static            │
        ├──────────────────────────────────────────────────────────┤
        │  Handlers (one per resource) — `src/handlers/`            │
        │   Auth, Ledgers, Accounts, Transactions, Documents,      │
        │   Reports, Dashboard                                      │
        ├──────────────────────────────────────────────────────────┤
        │  Application / use cases — `src/reports/`, `src/charts/` │
        │   Trial balance, balance sheet, P&L, cash flow, GL.      │
        │   Server-rendered SVG chart helpers.                      │
        ├──────────────────────────────────────────────────────────┤
        │  Domain (no I/O beyond sqlx types) — `src/domain/`        │
        │   Ledger, Account, Transaction, Posting, Document.       │
        │   Pure data + the double-entry invariant.                │
        ├──────────────────────────────────────────────────────────┤
        │  Storage — `src/storage/`                                 │
        │   FilesystemStore for uploaded documents.                 │
        └────────────────────────┬─────────────────────────────────┘
                                 │ sqlx (PostgreSQL)
        ┌────────────────────────▼─────────────────────────────────┐
        │  PostgreSQL  (migrations in `migrations/`)                │
        │   ledgers, accounts, transactions, postings,             │
        │   documents, tags, transaction_tags                      │
        └──────────────────────────────────────────────────────────┘
```

Layer rules (enforced by review, not by separate crates):

- `domain/` MUST NOT import `axum`, `tokio` (only `sqlx` types), or
  `askama`. Pure data + invariant.
- `reports/` and `charts/` MAY import `domain/`, `sqlx`, and pure math
  for SVG generation. They MUST NOT import `axum` types.
- `handlers/` MAY import `domain/`, `reports/`, `charts/`, `auth/`,
  `storage/`, `axum`. This is the only layer that touches the HTTP
  transport.
- `auth/` MAY import `axum-login` and `sqlx`. This is the only layer
  that knows about password hashing and sessions.
- `main.rs` is the composition root. It is the only file that
  constructs the `PgPool`, the `FilesystemStore`, the `AppState`, and
  wires the router.

### Tech stack

| Concern | Choice | Why |
|---|---|---|
| HTTP | Axum 0.8 | Ergonomic, tower-compatible |
| DB | PostgreSQL + sqlx | Real RDBMS, strong invariants |
| Sessions | tower-sessions (Postgres-backed) | No Redis to operate |
| Passwords | Argon2id | OWASP-recommended |
| Templates | Askama 0.13 | Pure-Rust, type-safe, Jinja-like |
| Charts | Hand-written SVG, server-rendered | No JS chart lib, no build |
| Frontend | HTMX + Tailwind Play CDN | Zero build, server-rendered partials |
| Documents | Local FS under `DOCUMENTS_DIR` | Simple, swappable later |

---

## 4. Test Discipline

Test categories (matches `openspec/specs/testing/spec.md` once archived):

| Category | Location | Naming |
|---|---|---|
| Unit | `src/<area>.rs` inside `#[cfg(test)] mod tests` | `tests::test_<unit>` |
| Integration | `tests/integration/<area>.rs` | `<area>_<behavior>` |
| Property | `mod prop` next to the domain code | `prop_<invariant>` |
| HTTP | `tests/http/<flow>.rs` | `http_<flow>_<scenario>` |

**Repeatability:** tests MUST NOT depend on:

- A running Postgres daemon (the test is skipped or uses `TestDb`).
- The current contents of any DB (each test gets a fresh DB).
- Filesystem state from a previous test.

Property-based tests for invariants (≥ 100 cases by default):

- Posting balance: any random N-leg transaction with random debits/credits
  where `Σ debits = Σ credits` is accepted; otherwise rejected.
- Sanitized filename never contains `..` or `/`.
- Argon2 password hash is bijective (round-trip succeeds).

---

## 5. Quality Engineering

- `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!` are
  **forbidden in production code** (`#[cfg(test)]` exempt).
  Use `?`, `match`, or `.expect("invariant: ...")` with a reason.
- `unsafe_code = "forbid"` in production crates.
- `#[allow(dead_code)]` is banned — delete or use the code.
- `cargo fmt --check` on every commit.
- `cargo clippy -- -D warnings` clean.
- `cargo doc` clean.

---

## 6. Configuration Layering

1. Built-in defaults in `config.rs`
2. `./.env` (or path from `dotenvy::dotenv()`)
3. Environment variables (`APP_HOST`, `APP_PORT`, `APP_SECRET`,
   `DATABASE_URL`, `DOCUMENTS_DIR`, `RUST_LOG`)

`APP_SECRET` MUST be ≥ 32 characters; startup fails otherwise.

---

## 7. Agent Workflow Checklist

When asked to implement a feature or spec:

1. **Read** the OpenSpec change folder: `proposal.md`, the cap's
   `specs/<cap>/spec.md`, `design.md`, `tasks.md`. These are
   authoritative.
2. **Check** `openspec/specs/` for any relevant existing capability.
3. **Plan** by walking `tasks.md` top-to-bottom. `## 1. Testing` first.
4. **Implement** in layer order: domain → app/reports/charts → handlers
   → templates. Add tests first; ensure they fail; add code; ensure
   they pass.
5. **Smoke-test** at the HTTP layer (`curl` against the local server).
6. **Run** `cargo fmt --check && cargo clippy -- -D warnings && cargo test`.
7. **Update** the change's `tasks.md` — every box checked.
8. **Validate** with `openspec validate <name>`.
9. **Archive** with `openspec archive <name>`.
10. **Commit** with a clear message: short title, blank line, *why* and
    *what* (not just *what*).

---

## 8. Anti-Patterns

- **Don't** write code without a corresponding OpenSpec change.
- **Don't** write a test that only verifies the current code's current
  output. It will pass when the code is wrong.
- **Don't** put `unwrap()` in production code. Use `?`.
- **Don't** silence clippy with `#[allow(dead_code)]`. Delete or use.
- **Don't** edit files outside the layer you own. If the auth layer
  needs a new field on User, the change must update the auth spec.
- **Don't** ship without `openspec validate` passing.
- **Don't** archive a change that has unchecked task boxes.

---

## 9. References

- `openspec/changes/2026-08-13-bootstrap-double-entry-bookkeeping-engine/` — first change
- `openspec/specs/bookkeeping/spec.md` — double-entry model contract
- `openspec/specs/auth/spec.md` — auth model contract
- `openspec/specs/documents/spec.md` — document storage contract
- `openspec/specs/reports/spec.md` — report computation contract
- `openspec/specs/web-ui/spec.md` — responsive UI contract
- `openspec/specs/architecture/spec.md` — layered architecture contract
- `openspec/specs/testing/spec.md` — test discipline (TBD)
- `openspec/specs/quality/spec.md` — quality policy (TBD)
- `NOTICE` — third-party attributions (vito, htmx, tailwind)
- `README.md` — project overview, vito credit, first-principles
