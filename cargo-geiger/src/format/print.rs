use crate::args::Args;
use crate::format::pattern::Pattern;
use crate::format::{Charset, CrateDetectionStatus, FormatError};

use cargo::core::shell::Verbosity;
use cargo::util::errors::CliError;
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

pub struct PrintConfig {
    /// Don't truncate dependencies that have already been displayed.
    pub all: bool,

    pub allow_partial_results: bool,
    pub charset: Charset,
    pub direction: EdgeDirection,

    // Is anyone using this? This is a carry-over from cargo-tree.
    // TODO: Open a github issue to discuss deprecation.
    pub format: Pattern,

    pub include_tests: IncludeTests,
    pub prefix: Prefix,
    pub output_format: Option<OutputFormat>,
    pub verbosity: Verbosity,
}

impl PrintConfig {
    pub fn new(args: &Args) -> Result<Self, CliError> {
        // TODO: Add command line flag for this and make it default to false?
        let allow_partial_results = true;

        let direction = if args.invert {
            EdgeDirection::Incoming
        } else {
            EdgeDirection::Outgoing
        };

        let format = Pattern::try_build(&args.format).map_err(|e| {
            CliError::new(
                (FormatError {
                    message: e.to_string(),
                })
                .into(),
                1,
            )
        })?;

        let include_tests = if args.include_tests {
            IncludeTests::Yes
        } else {
            IncludeTests::No
        };

        let prefix = if args.prefix_depth {
            Prefix::Depth
        } else if args.no_indent {
            Prefix::None
        } else {
            Prefix::Indent
        };

        let verbosity = if args.verbose == 0 {
            Verbosity::Normal
        } else {
            Verbosity::Verbose
        };

        Ok(PrintConfig {
            all: args.all,
            allow_partial_results,
            charset: args.charset,
            direction,
            format,
            include_tests,
            output_format: args.output_format,
            prefix,
            verbosity,
        })
    }
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
