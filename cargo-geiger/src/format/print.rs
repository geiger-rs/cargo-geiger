use crate::format::{Charset, CrateDetectionStatus, Pattern};

use cargo::core::shell::Verbosity;
use colored::Colorize;
use geiger::IncludeTests;
use petgraph::EdgeDirection;

#[derive(Clone, Copy)]
pub enum Prefix {
    None,
    Indent,
    Depth,
}

pub struct PrintConfig<'a> {
    /// Don't truncate dependencies that have already been displayed.
    pub all: bool,

    pub verbosity: Verbosity,
    pub direction: EdgeDirection,
    pub prefix: Prefix,

    // Is anyone using this? This is a carry-over from cargo-tree.
    // TODO: Open a github issue to discuss deprecation.
    pub format: &'a Pattern,

    pub charset: Charset,
    pub allow_partial_results: bool,
    pub include_tests: IncludeTests,
}

pub fn colorize(
    s: String,
    detection_status: &CrateDetectionStatus,
) -> colored::ColoredString {
    match detection_status {
        CrateDetectionStatus::NoneDetectedForbidsUnsafe => s.green(),
        CrateDetectionStatus::NoneDetectedAllowsUnsafe => s.normal(),
        CrateDetectionStatus::UnsafeDetected => s.red().bold(),
    }
}
