use crate::format::print_config::OutputFormat;
use crate::format::Charset;

use pico_args::Arguments;
use std::path::PathBuf;

pub const HELP: &str =
    "Detects usage of unsafe Rust in a Rust crate and its dependencies.

USAGE:
    cargo geiger [OPTIONS]

OPTIONS:
    -p, --package <SPEC>          Package to be used as the root of the tree.
        --features <FEATURES>     Space-separated list of features to activate.
        --all-features            Activate all available features.
        --no-default-features     Do not activate the `default` feature.
        --target <TARGET>         Set the target triple.
        --all-targets             Return dependencies for all targets. By
                                  default only the host target is matched.
        --manifest-path <PATH>    Path to Cargo.toml.
    -i, --invert                  Invert the tree direction.
        --no-indent               Display the dependencies as a list (rather
                                  than a tree).
        --prefix-depth            Display the dependencies as a list (rather
                                  than a tree), but prefixed with the depth.
    -a, --all                     Don't truncate dependencies that have already
                                  been displayed.
        --charset <CHARSET>       Character set to use in output: utf8, ascii
                                  [default: utf8].
    --format <FORMAT>             Format string used for printing dependencies
                                  [default: {p}].
    --json                        Output in JSON format.
    -v, --verbose                 Use verbose output (-vv very verbose/build.rs
                                  output).
    -q, --quiet                   No output printed to stdout other than the
                                  tree.
        --color <WHEN>            Coloring: auto, always, never.
        --frozen                  Require Cargo.lock and cache are up to date.
        --locked                  Require Cargo.lock is up to date.
        --offline                 Run without accessing the network.
    -Z \"<FLAG>...\"                Unstable (nightly-only) flags to Cargo.
        --include-tests           Count unsafe usage in tests..
        --build-dependencies      Also analyze build dependencies.
        --dev-dependencies        Also analyze dev dependencies.
        --all-dependencies        Analyze all dependencies, including build and
                                  dev.
        --forbid-only             Don't build or clean anything, only scan
                                  entry point .rs source files for.
                                  forbid(unsafe_code) flags. This is
                                  significantly faster than the default
                                  scanning mode. TODO: Add ability to combine
                                  this with a whitelist for use in CI.
    -h, --help                    Prints help information.
    -V, --version                 Prints version information.
";

pub struct Args {
    pub all: bool,
    pub all_deps: bool,
    pub all_features: bool,
    pub all_targets: bool,
    pub build_deps: bool,
    pub charset: Charset,
    pub color: Option<String>,
    pub dev_deps: bool,
    pub features: Option<String>,
    pub forbid_only: bool,
    pub format: String,
    pub frozen: bool,
    pub help: bool,
    pub include_tests: bool,
    pub invert: bool,
    pub locked: bool,
    pub manifest_path: Option<PathBuf>,
    pub no_default_features: bool,
    pub no_indent: bool,
    pub offline: bool,
    pub package: Option<String>,
    pub prefix_depth: bool,
    pub quiet: bool,
    pub target: Option<String>,
    pub unstable_flags: Vec<String>,
    pub verbose: u32,
    pub version: bool,
    pub output_format: Option<OutputFormat>,
}

impl Args {
    pub fn parse_args(
        mut raw_args: Arguments,
    ) -> Result<Args, Box<dyn std::error::Error>> {
        let args = Args {
            all: raw_args.contains(["-a", "--all"]),
            all_deps: raw_args.contains("--all-dependencies"),
            all_features: raw_args.contains("--all-features"),
            all_targets: raw_args.contains("--all-targets"),
            build_deps: raw_args.contains("--build-dependencies"),
            charset: raw_args
                .opt_value_from_str("--charset")?
                .unwrap_or(Charset::Utf8),
            color: raw_args.opt_value_from_str("--color")?,
            dev_deps: raw_args.contains("--dev-dependencies"),
            features: raw_args.opt_value_from_str("--features")?,
            forbid_only: raw_args.contains(["-f", "--forbid-only"]),
            format: raw_args
                .opt_value_from_str("--format")?
                .unwrap_or_else(|| "{p}".to_string()),
            frozen: raw_args.contains("--frozen"),
            help: raw_args.contains(["-h", "--help"]),
            include_tests: raw_args.contains("--include-tests"),
            invert: raw_args.contains(["-i", "--invert"]),
            locked: raw_args.contains("--locked"),
            manifest_path: raw_args.opt_value_from_str("--manifest-path")?,
            no_default_features: raw_args.contains("--no-default-features"),
            no_indent: raw_args.contains("--no-indent"),
            offline: raw_args.contains("--offline"),
            package: raw_args.opt_value_from_str("--manifest-path")?,
            prefix_depth: raw_args.contains("--prefix-depth"),
            quiet: raw_args.contains(["-q", "--quiet"]),
            target: raw_args.opt_value_from_str("--target")?,
            unstable_flags: raw_args
                .opt_value_from_str("-Z")?
                .map(|s: String| s.split(' ').map(|s| s.to_owned()).collect())
                .unwrap_or_else(Vec::new),
            verbose: match (
                raw_args.contains("-vv"),
                raw_args.contains(["-v", "--verbose"]),
            ) {
                (false, false) => 0,
                (false, true) => 1,
                (true, _) => 2,
            },
            version: raw_args.contains(["-V", "--version"]),
            output_format: if raw_args.contains("--json") {
                Some(OutputFormat::Json)
            } else {
                None
            },
        };
        Ok(args)
    }
}

#[cfg(test)]
pub mod args_tests {
    use super::*;

    use rstest::*;
    use std::ffi::OsString;

    #[rstest(
        input_argument_vector,
        expected_all,
        expected_charset,
        expected_verbose,
        case(
            vec![],
            false,
            Charset::Utf8,
            0
        ),
        case(
            vec![OsString::from("--all")],
            true,
            Charset::Utf8,
            0,
        ),
        case(
            vec![OsString::from("--charset"), OsString::from("ascii")],
            false,
            Charset::Ascii,
            0
        ),
        case(
            vec![OsString::from("-v")],
            false,
            Charset::Utf8,
            1
        ),
        case(
            vec![OsString::from("-vv")],
            false,
            Charset::Utf8,
            2
        )
    )]
    fn parse_args_test(
        input_argument_vector: Vec<OsString>,
        expected_all: bool,
        expected_charset: Charset,
        expected_verbose: u32,
    ) {
        let args_result =
            Args::parse_args(Arguments::from_vec(input_argument_vector));

        assert!(args_result.is_ok());

        let args = args_result.unwrap();

        assert_eq!(args.all, expected_all);
        assert_eq!(args.charset, expected_charset);
        assert_eq!(args.verbose, expected_verbose)
    }
}
