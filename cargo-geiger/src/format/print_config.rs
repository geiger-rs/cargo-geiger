use crate::args::Args;
use crate::format::pattern::Pattern;
use crate::format::{CrateDetectionStatus, FormatError};

use cargo::core::shell::Verbosity;
use cargo::util::errors::CliError;
use colored::{ColoredString, Colorize};
use geiger::IncludeTests;
use petgraph::EdgeDirection;
use strum_macros::EnumString;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prefix {
    Depth,
    Indent,
    None,
}

#[derive(Clone, Copy, Debug, EnumString, Eq, PartialEq)]
pub enum OutputFormat {
    Ascii,
    Json,
    GitHubMarkdown,
    Ratio,
    Utf8,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Utf8
    }
}

#[derive(Debug, PartialEq)]
pub struct PrintConfig {
    /// Don't truncate dependencies that have already been displayed.
    pub all: bool,

    pub allow_partial_results: bool,
    pub direction: EdgeDirection,

    // Is anyone using this? This is a carry-over from cargo-tree.
    // TODO: Open a github issue to discuss deprecation.
    pub format: Pattern,

    pub include_tests: IncludeTests,
    pub prefix: Prefix,
    pub output_format: OutputFormat,
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
    crate_detection_status: &CrateDetectionStatus,
    output_format: OutputFormat,
    string: String,
) -> ColoredString {
    match output_format {
        OutputFormat::GitHubMarkdown => ColoredString::from(string.as_str()),
        _ => match crate_detection_status {
            CrateDetectionStatus::NoneDetectedForbidsUnsafe => string.green(),
            CrateDetectionStatus::NoneDetectedAllowsUnsafe => string.normal(),
            CrateDetectionStatus::UnsafeDetected => string.red().bold(),
        },
    }
}

#[cfg(test)]
mod print_config_tests {
    use super::*;

    use rstest::*;
    use std::str::FromStr;

    #[rstest(
        input_invert_bool,
        expected_edge_direction,
        case(true, EdgeDirection::Incoming),
        case(false, EdgeDirection::Outgoing)
    )]
    fn print_config_new_test_invert(
        input_invert_bool: bool,
        expected_edge_direction: EdgeDirection,
    ) {
        let args = Args {
            invert: input_invert_bool,
            ..Default::default()
        };

        let print_config_result = PrintConfig::new(&args);

        assert!(print_config_result.is_ok());
        assert_eq!(
            print_config_result.unwrap().direction,
            expected_edge_direction
        );
    }

    #[rstest(
        input_include_tests_bool,
        expected_include_tests,
        case(true, IncludeTests::Yes),
        case(false, IncludeTests::No)
    )]
    fn print_config_new_test_include_tests(
        input_include_tests_bool: bool,
        expected_include_tests: IncludeTests,
    ) {
        let args = Args {
            include_tests: input_include_tests_bool,
            ..Default::default()
        };

        let print_config_result = PrintConfig::new(&args);

        assert!(print_config_result.is_ok());
        assert_eq!(
            print_config_result.unwrap().include_tests,
            expected_include_tests
        );
    }

    #[rstest(
        input_prefix_depth_bool,
        input_no_indent_bool,
        expected_output_prefix,
        case(true, false, Prefix::Depth,),
        case(true, false, Prefix::Depth,),
        case(false, true, Prefix::None,),
        case(false, false, Prefix::Indent,)
    )]
    fn print_config_new_test_prefix(
        input_prefix_depth_bool: bool,
        input_no_indent_bool: bool,
        expected_output_prefix: Prefix,
    ) {
        let args = Args {
            no_indent: input_no_indent_bool,
            prefix_depth: input_prefix_depth_bool,
            ..Default::default()
        };

        let print_config_result = PrintConfig::new(&args);

        assert!(print_config_result.is_ok());
        assert_eq!(print_config_result.unwrap().prefix, expected_output_prefix);
    }

    #[rstest(
        input_verbosity_u32,
        expected_verbosity,
        case(0, Verbosity::Normal),
        case(1, Verbosity::Verbose),
        case(1, Verbosity::Verbose)
    )]
    fn print_config_new_test_verbosity(
        input_verbosity_u32: u32,
        expected_verbosity: Verbosity,
    ) {
        let args = Args {
            verbose: input_verbosity_u32,
            ..Default::default()
        };

        let print_config_result = PrintConfig::new(&args);

        assert!(print_config_result.is_ok());
        assert_eq!(print_config_result.unwrap().verbosity, expected_verbosity);
    }

    #[rstest(
        input_raw_str,
        expected_output_format_result,
        case("Ascii", Ok(OutputFormat::Ascii)),
        case("Json", Ok(OutputFormat::Json)),
        case("GitHubMarkdown", Ok(OutputFormat::GitHubMarkdown)),
        case("Utf8", Ok(OutputFormat::Utf8)),
        case("unknown_variant", Err(strum::ParseError::VariantNotFound))
    )]
    fn output_format_from_str_test(
        input_raw_str: &str,
        expected_output_format_result: Result<OutputFormat, strum::ParseError>,
    ) {
        let output_format = OutputFormat::from_str(input_raw_str);
        assert_eq!(output_format, expected_output_format_result);
    }

    #[rstest(
        input_crate_detection_status,
        input_output_format,
        expected_colored_string,
        case(
            CrateDetectionStatus::NoneDetectedForbidsUnsafe,
            OutputFormat::Ascii,
            String::from("string_value").green()
        ),
        case(
            CrateDetectionStatus::NoneDetectedAllowsUnsafe,
            OutputFormat::Utf8,
            String::from("string_value").normal()
        ),
        case(
            CrateDetectionStatus::UnsafeDetected,
            OutputFormat::Ascii,
            String::from("string_value").red().bold()
        ),
        case(
            CrateDetectionStatus::NoneDetectedForbidsUnsafe,
            OutputFormat::GitHubMarkdown,
            ColoredString::from("string_value")
        ),
        case(
            CrateDetectionStatus::NoneDetectedAllowsUnsafe,
            OutputFormat::GitHubMarkdown,
            ColoredString::from("string_value")
        ),
        case(
            CrateDetectionStatus::UnsafeDetected,
            OutputFormat::GitHubMarkdown,
            ColoredString::from("string_value")
        )
    )]
    fn colorize_test(
        input_crate_detection_status: CrateDetectionStatus,
        input_output_format: OutputFormat,
        expected_colored_string: ColoredString,
    ) {
        let string_value = String::from("string_value");

        assert_eq!(
            colorize(
                &input_crate_detection_status,
                input_output_format,
                string_value
            ),
            expected_colored_string
        );
    }
}
