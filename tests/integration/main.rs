// Integration tests are test code. The production lints forbid
// `unwrap` / `expect` / `panic`; the test code is allowed to use
// them, and the fixture itself relies on them for clean failure
// modes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../common/mod.rs"]
mod common;

mod cash_basis;
mod smoke;
