# account Specification (delta)

## ADDED Requirements

### Requirement: Profile View

The system SHALL expose `GET /account` to authenticated users. The
rendered page MUST display, at minimum:

- `username` (read-only, the value the user registered with)
- `email` (read-only)
- `display_name` (read-only; rendered as `—` when `NULL`)
- `created_at` formatted as `YYYY-MM-DD`
- `updated_at` formatted as `YYYY-MM-DD HH:MM`

Unauthenticated requests to `GET /account` MUST redirect to
`/login?next=/account` (this is the existing `login_required!`
layer's behaviour; we do not add a special case).

#### Scenario: Authenticated user opens the page

- **WHEN** an authenticated user requests `GET /account`
- **THEN** the response is `200 OK` and the HTML body contains the
  user's `username`, `email`, `display_name` (or `—` if null), the
  formatted `created_at` date, and the formatted `updated_at`
  timestamp.

#### Scenario: Anonymous user opens the page

- **WHEN** an unauthenticated client requests `GET /account`
- **THEN** the response is a `303 See Other` redirect to
  `/login?next=/account`. No user data is leaked in the response.

### Requirement: Password Change

The system SHALL expose `POST /account/password`. The handler MUST
verify, in order:

1. The caller is authenticated (enforced by the existing
   `login_required!` layer).
2. The submitted `current_password` verifies against the stored
   Argon2id hash for the authenticated user. If verification fails,
   the response is `401 Unauthorized` with the body
   `Current password is incorrect.`
3. The submitted `new_password` is at least 8 characters long. If
   not, the response is `400 Bad Request` with the body
   `New password must be at least 8 characters.`
4. The submitted `new_password` and `confirm_password` are equal.
   If not, the response is `400 Bad Request` with the body
   `New password and confirmation do not match.`
5. The new password is hashed with Argon2id (fresh salt, default
   parameters) and stored via
   `UPDATE users SET hashed_password = $1, updated_at = now() WHERE id = $2`.

On success, the response is `303 See Other` redirecting to `/account`,
and the page is re-rendered with a flash banner
`Password updated.` (kept until the next request).

The new password MUST be required to take effect before the next
request: re-issuing `POST /login` with the new password MUST succeed;
re-issuing with the old password MUST fail with `Invalid email or
password.`

#### Scenario: Wrong current password is rejected

- **WHEN** an authenticated user submits
  `POST /account/password` with `current_password` that does not
  match their stored hash
- **THEN** the response is `401 Unauthorized` with the body
  `Current password is incorrect.` No row in `users` is updated.

#### Scenario: New password too short is rejected

- **WHEN** an authenticated user submits a new password shorter than
  8 characters (with a correct current password)
- **THEN** the response is `400 Bad Request` with the body
  `New password must be at least 8 characters.` No row in
  `users` is updated.

#### Scenario: Mismatched confirmation is rejected

- **WHEN** an authenticated user submits `new_password` and
  `confirm_password` that differ (with a correct current password)
- **THEN** the response is `400 Bad Request` with the body
  `New password and confirmation do not match.` No row in
  `users` is updated.

#### Scenario: Happy path persists the new password

- **WHEN** an authenticated user submits a correct current password
  and a new password that is ≥ 8 characters and matches its
  confirmation
- **THEN** the response is `303 See Other` with `location: /account`.
  The `users.hashed_password` row for that user is updated, and
  `updated_at` is set to the current time.

#### Scenario: Old password stops working after rotation

- **WHEN** user U successfully changes their password from `oldpw`
  to `newpw`
- **THEN** `POST /login` with `password=newpw` succeeds and returns
  a session cookie, and `POST /login` with `password=oldpw` fails
  with `Invalid email or password.`

### Requirement: No Information Disclosure

The error messages returned by `POST /account/password` MUST NOT
distinguish between "user does not exist", "user is disabled", and
"current password is wrong" in any way that lets an attacker
enumerate accounts. Concretely:

- For a non-existent user, the handler MUST return the same
  status code and the same body as for a wrong-password attempt on
  an existing user.
- Timing of the response MUST NOT measurably differ between the
  two cases.

#### Scenario: Anonymous attacker cannot enumerate

- **WHEN** an attacker submits `POST /account/password` for a
  username that does not exist
- **THEN** the response is the same `401 Unauthorized` with the
  same body as a wrong-password submission would receive, and the
  response is returned within ±50 ms of the wrong-password
  response time for an existing user.
