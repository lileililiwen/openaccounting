//! Bank-statement import: format-agnostic entry point that
//! dispatches to the OFX / QIF / MT940 parsers and re-exports
//! the row type from the existing CSV importer.

pub mod mt940;
pub mod ofx;
pub mod qif;
pub mod sniff;

pub use mt940::Mt940;
pub use sniff::Format;
