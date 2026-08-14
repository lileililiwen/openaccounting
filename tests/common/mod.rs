//! Re-exports the per-test fixtures from the library so integration
//! tests can write `use crate::common::*;` and get `TestServer`,
//! `TestDb`, etc.
//!
//! The library is built with the `test-support` feature in
//! `[dev-dependencies]` (see Cargo.toml `[[test]] integration`
//! block), so `openaccounting::test_support` is in scope here.

// Both are part of the public test surface; future integration
// test files will use them. The re-export itself is the point.
#![allow(unused_imports)]
pub use openaccounting::test_support::{TestDb, TestServer};
