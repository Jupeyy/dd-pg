//! This crate contains a interfaces for common tasks
//! on the database.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::nursery)]
#![deny(clippy::all)]

/// Everything related to queries
pub mod query;
/// Everything related to versioning table setups
pub mod version;
