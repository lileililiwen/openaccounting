# auth Specification (delta)

## ADDED Requirements

### Requirement: User Registration

A first-time visitor MAY create an account by providing
`username`, `email`, and `password` (plus `password_confirm`). The
handler MUST:

- Reject if `password` is shorter than 8 characters.
- Reject if `password != password_confirm`.
- Reject if `email` is not RFC-5321-valid (basic check).
- Reject if `username` is shorter than 3 characters or contains
  characters outside `[a-zA-Z0-9_-]`.
- Hash the password with **Argon2id** using `Argon2::default()` and a
  fresh random `SaltString` per user.
- Insert the user with `is_active = TRUE`.
- Redirect to `/login?next=/ledgers/new`.

The plaintext password MUST NOT appear in any log, error message, or
HTTP response. The `hashed_password` column is marked `SERIAL`-style
"never returned to callers" — handlers MUST NOT serialize it.

#### Scenario: Successful registration

- **WHEN** a visitor submits valid form data
- **THEN** the response is HTTP 303 redirecting to
  `/login?next=/ledgers/new`. The `users` table contains one new row
  whose `hashed_password` starts with `$argon2`.

#### Scenario: Password mismatch

- **WHEN** the form submits `password=foo12345` and
  `password_confirm=foo12346`
- **THEN** the response is HTTP 200 with the form re-rendered and
  the error banner "Passwords do not match".

#### Scenario: Duplicate email

- **WHEN** the form submits an email that already exists
- **THEN** the response is HTTP 200 with the form re-rendered and
  the error banner "email or username already in use".

### Requirement: User Login

A registered user MAY sign in by providing `email` and `password`.
The handler MUST:

- Look up the user by `LOWER(email) = LOWER($1)`.
- Reject if the user does not exist OR is `is_active = FALSE`.
- Reject if the password does not verify against the stored Argon2id
  hash.
- On success, create a session via `axum-login` with a 30-day cookie
  (`tower-sessions`) and redirect to `next` (or `/` if absent).

Failed logins MUST return a generic error ("Invalid email or
password") and MUST NOT reveal which field was wrong.

#### Scenario: Successful login

- **WHEN** a user submits the correct `email` and `password`
- **THEN** the response is HTTP 303 with a `Set-Cookie: oa_session=…`
  header and a redirect to the requested `next` URL (or `/`).

#### Scenario: Wrong password

- **WHEN** a user submits the correct `email` and a wrong `password`
- **THEN** the response is HTTP 200 with the form re-rendered and
  the error banner "Invalid email or password". No session is
  created.

### Requirement: Session Storage

Sessions SHALL be stored in PostgreSQL via
`tower-sessions-sqlx-store::PostgresStore`. The session cookie is
named `oa_session`, marked `HttpOnly`, `SameSite=Lax`, and lasts 30
days. In development the `Secure` flag is `false`; production
deployments MUST set it to `true` (documented in `README.md`).

The session is invalidated when the user logs out, when the cookie
expires, or when the session record is removed by a server-side
cleanup job (out of scope for v0.1).

#### Scenario: Logout invalidates the session

- **WHEN** a logged-in user submits `POST /logout`
- **THEN** the response is HTTP 303 to `/login` with a
  `Set-Cookie: oa_session=; Max-Age=0` header. Subsequent requests
  to protected routes are redirected to `/login`.

### Requirement: Route Protection

All routes under `/ledgers/*`, `/transactions/*`, `/accounts/*`,
`/documents/*`, `/reports/*`, and `/dashboard` MUST be wrapped by
`axum-login`'s `login_required!(Backend)` layer. Unauthenticated
requests MUST be redirected to `/login?next=<original-url>`.

Public routes are: `/`, `/login`, `/register`, `/static/*`, and
`/healthz` (the latter is reserved for a future change).

#### Scenario: Anonymous request to a protected route

- **WHEN** an anonymous user requests `GET /ledgers/123/dashboard`
- **THEN** the response is HTTP 303 to
  `/login?next=/ledgers/123/dashboard`.
