//! The outer CLI parts of the `cargo-geiger` cargo plugin executable.
//! TODO: Refactor this file to only deal with command line argument processing.

#![forbid(unsafe_code)]
#![forbid(warnings)]

extern crate cargo;
extern crate colored;
extern crate petgraph;

mod cli;
mod find;
mod format;
mod graph;
mod rs_file;
mod scan;
mod traversal;

use crate::cli::get_cfgs;
use crate::cli::get_registry;
use crate::cli::get_workspace;
use crate::cli::resolve;
use crate::format::print::Prefix;
use crate::format::print::PrintConfig;
use crate::format::{Charset, Pattern};
use crate::graph::build_graph;
use crate::graph::ExtraDeps;
use crate::scan::{run_scan_mode_default, run_scan_mode_forbid_only};

use cargo::core::shell::{Shell, Verbosity};
use cargo::util::errors::CliError;
use cargo::CliResult;
use cargo::Config;
use geiger::IncludeTests;
use petgraph::EdgeDirection;
use std::fmt;
use std::path::PathBuf;

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

const HELP: &str =
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
    pub quiet: Option<bool>,
    pub target: Option<String>,
    pub unstable_flags: Vec<String>,
    pub verbose: u32,
    pub version: bool,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut args = pico_args::Arguments::from_env();
    let args = Args {
        all: args.contains(["-a", "--all"]),
        all_deps: args.contains("--all-dependencies"),
        all_features: args.contains("--all-features"),
        all_targets: args.contains("--all-targets"),
        build_deps: args.contains("--build-dependencies"),
        charset: args
            .opt_value_from_str("--charset")?
            .unwrap_or(Charset::Utf8),
        color: args.opt_value_from_str("--color")?,
        dev_deps: args.contains("--dev-dependencies"),
        features: args.opt_value_from_str("--features")?,
        forbid_only: args.contains(["-f", "--forbid-only"]),
        format: args
            .opt_value_from_str("--format")?
            .unwrap_or_else(|| "{p}".to_string()),
        frozen: args.contains("--frozen"),
        help: args.contains(["-h", "--help"]),
        include_tests: args.contains("--include-tests"),
        invert: args.contains(["-i", "--invert"]),
        locked: args.contains("--locked"),
        manifest_path: args.opt_value_from_str("--manifest-path")?,
        no_default_features: args.contains("--no-default-features"),
        no_indent: args.contains("--no-indent"),
        offline: args.contains("--offline"),
        package: args.opt_value_from_str("--manifest-path")?,
        prefix_depth: args.contains("--prefix-depth"),
        quiet: args.opt_value_from_str(["-q", "--quiet"])?,
        target: args.opt_value_from_str("--target")?,
        unstable_flags: args
            .opt_value_from_str("-Z")?
            .map(|s: String| s.split(' ').map(|s| s.to_owned()).collect())
            .unwrap_or_else(Vec::new),
        verbose: match (
            args.contains("-vv"),
            args.contains(["-v", "--verbose"]),
        ) {
            (false, false) => 0,
            (false, true) => 1,
            (true, _) => 2,
        },
        version: args.contains(["-V", "--version"]),
    };
    Ok(args)
}

#[derive(Debug)]
struct FormatError {
    message: String,
}

impl std::error::Error for FormatError {}

/// Forward Display to Debug, probably good enough for programmer facing error
/// messages.
impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

fn real_main(args: &Args, config: &mut Config) -> CliResult {
    use cargo::core::shell::ColorChoice;

    if args.version {
        println!("cargo-geiger {}", VERSION.unwrap_or("unknown version"));
        return Ok(());
    }
    if args.help {
        println!("{}", HELP);
        return Ok(());
    }
    let target_dir = None; // Doesn't add any value for cargo-geiger.
    config.configure(
        args.verbose,
        args.quiet.unwrap_or(false),
        args.color.as_deref(),
        args.frozen,
        args.locked,
        args.offline,
        &target_dir,
        &args.unstable_flags,
        &[], // Some cargo API change, TODO: Look closer at this later.
    )?;
    let verbosity = if args.verbose == 0 {
        Verbosity::Normal
    } else {
        Verbosity::Verbose
    };
    match config.shell().color_choice() {
        ColorChoice::Always => colored::control::set_override(true),
        ColorChoice::Never => colored::control::set_override(false),
        ColorChoice::CargoAuto => {}
    }

    let ws = get_workspace(config, args.manifest_path.clone())?;
    let package = ws.current()?;
    let mut registry = get_registry(config, &package)?;
    let features = args
        .features
        .as_ref()
        .cloned()
        .unwrap_or_else(String::new)
        .split(' ')
        .map(str::to_owned)
        .collect::<Vec<String>>();
    let (packages, resolve) = resolve(
        package.package_id(),
        &mut registry,
        &ws,
        &features,
        args.all_features,
        args.no_default_features,
    )?;
    let ids = packages.package_ids().collect::<Vec<_>>();
    let packages = registry.get(&ids)?;

    let root_pack_id = match args.package {
        Some(ref pkg) => resolve.query(pkg)?,
        None => package.package_id(),
    };

    let config_host = config.load_global_rustc(Some(&ws))?.host;
    let target = if args.all_targets {
        None
    } else {
        Some(args.target.as_deref().unwrap_or(&config_host))
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

    let extra_deps = if args.all_deps {
        ExtraDeps::All
    } else if args.build_deps {
        ExtraDeps::Build
    } else if args.dev_deps {
        ExtraDeps::Dev
    } else {
        ExtraDeps::NoMore
    };

    let cfgs = get_cfgs(config, &args.target, &ws)?;
    let graph = build_graph(
        &resolve,
        &packages,
        package.package_id(),
        target,
        cfgs.as_deref(),
        extra_deps,
    )?;

    let direction = if args.invert {
        EdgeDirection::Incoming
    } else {
        EdgeDirection::Outgoing
    };

    let prefix = if args.prefix_depth {
        Prefix::Depth
    } else if args.no_indent {
        Prefix::None
    } else {
        Prefix::Indent
    };

    // TODO: Add command line flag for this and make it default to false?
    let allow_partial_results = true;

    let include_tests = if args.include_tests {
        IncludeTests::Yes
    } else {
        IncludeTests::No
    };
    let pc = PrintConfig {
        all: args.all,
        verbosity,
        direction,
        prefix,
        format: &format,
        charset: args.charset,
        allow_partial_results,
        include_tests,
    };

    if args.forbid_only {
        run_scan_mode_forbid_only(&config, &packages, root_pack_id, &graph, &pc)
    } else {
        run_scan_mode_default(
            &config,
            &ws,
            &packages,
            root_pack_id,
            &graph,
            &pc,
            &args,
        )
    }
}

fn main() {
    env_logger::init();
    let mut config = match Config::default() {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e.into(), &mut shell)
        }
    };
    let args = parse_args().unwrap();
    if let Err(e) = real_main(&args, &mut config) {
        let mut shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}
