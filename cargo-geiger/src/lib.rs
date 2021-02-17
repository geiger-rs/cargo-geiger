//! These modules expose the internal workings of `cargo-geiger`. They
//! are currently not stable, and therefore have no associated `SemVer`.
//! As such, any function contained within may be subject to change.

#![deny(clippy::cargo)]
#![deny(clippy::doc_markdown)]
#![forbid(unsafe_code)]
#![deny(warnings)]

/// Argument parsing
pub mod args;
/// Bootstrapping functions for structs required by the CLI
pub mod cli;
/// Construction of the dependency graph
pub mod graph;
/// Mapping functionality from `cargo::core` to `cargo_metadata`
pub mod mapping;
/// Interaction with README.md files
pub mod readme;
/// Functions for scanning projects for unsafe code
pub mod scan;

/// Inner display formatting
mod format;
/// Tree construction
mod tree;
