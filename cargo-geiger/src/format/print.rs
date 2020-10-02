use crate::format::pattern::Pattern;
use crate::format::{Charset, CrateDetectionStatus};

use cargo::core::shell::Verbosity;
use colored::Colorize;
use geiger::IncludeTests;
use petgraph::EdgeDirection;

#[derive(Clone, Copy)]
pub enum Prefix {
    Depth,
    Indent,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Json,
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
    pub output_format: Option<OutputFormat>,
}

pub fn colorize(
    string: String,
    crate_detection_status: &CrateDetectionStatus,
) -> colored::ColoredString {
    match crate_detection_status {
        CrateDetectionStatus::NoneDetectedForbidsUnsafe => string.green(),
        CrateDetectionStatus::NoneDetectedAllowsUnsafe => string.normal(),
        CrateDetectionStatus::UnsafeDetected => string.red().bold(),
    }
}

#[cfg(test)]
mod print_tests {
    use super::*;

    #[test]
    fn colorize_test() {
        let string = String::from("string_value");

        assert_eq!(
            string.clone().green(),
            colorize(
                string.clone(),
                &CrateDetectionStatus::NoneDetectedForbidsUnsafe
            )
        );

        assert_eq!(
            string.clone().normal(),
            colorize(
                string.clone(),
                &CrateDetectionStatus::NoneDetectedAllowsUnsafe
            )
        );

        assert_eq!(
            string.clone().red().bold(),
            colorize(string.clone(), &CrateDetectionStatus::UnsafeDetected)
        );
    }
}
