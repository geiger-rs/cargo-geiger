//! cargo-geiger-serde ☢
//! ========
//!
//! This crate provides definitions to serialize the unsafety report.

#![forbid(unsafe_code)]
#![forbid(warnings)]

mod package_id;
mod report;
mod source;

pub use package_id::PackageId;
pub use report::{
    Count, CounterBlock, DependencyKind, PackageInfo, QuickReportEntry,
    QuickSafetyReport, ReportEntry, SafetyReport, UnsafeInfo,
};
pub use source::Source;
