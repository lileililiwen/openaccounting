# Tasks

## 1. Testing

- [ ] 1.1 Define the smoke test cases and stable semantic response assertions before implementation.
- [ ] 1.2 Add property-test cases for balanced/unbalanced postings, account sign mapping, and filename sanitization.
- [ ] 1.3 Add focused HTTP tests for authorization, CSRF, upload validation, API auth, and export boundaries.
- [ ] 1.4 Add a repeatability check that runs the smoke path twice with fresh databases and temporary storage.

## 2. Implementation

- [ ] 2.1 Add the `tests/smoke.rs` or equivalent `tests/http/` entry point using `TestServer` and `TestDb`.
- [ ] 2.2 Add generators and assertions for the three required pure invariants.
- [ ] 2.3 Add missing route-level setup helpers without duplicating authentication or database fixture logic.
- [ ] 2.4 Integrate smoke and property commands into CI without weakening existing gates.

## 3. Verification

- [ ] 3.1 Run each new focused test in isolation.
- [ ] 3.2 Run the smoke test twice against fresh databases.
- [ ] 3.3 Run the full library and integration suites.
- [ ] 3.4 Confirm coverage artifacts are uploaded and the configured floor remains enforced.
