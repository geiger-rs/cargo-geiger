//! The outer CLI parts of the `cargo-geiger` cargo plugin executable.
//! TODO: Refactor this file to only deal with command line argument processing.

#![forbid(unsafe_code)]
#![forbid(warnings)]

extern crate cargo;
extern crate colored;
extern crate petgraph;
extern crate structopt;

mod cli;
mod format;

use crate::cli::build_graph;
use crate::cli::get_cfgs;
use crate::cli::get_registry;
use crate::cli::get_workspace;
use crate::cli::resolve;
use crate::cli::run_scan_mode_default;
use crate::cli::run_scan_mode_forbid_only;
use crate::cli::Charset;
use crate::cli::ExtraDeps;
use crate::cli::Prefix;
use crate::cli::PrintConfig;
use crate::format::Pattern;
use cargo::core::shell::Shell;
use cargo::core::shell::Verbosity;
use cargo::util::errors::CliError;
use cargo::CliResult;
use cargo::Config;
use geiger::IncludeTests;
use petgraph::EdgeDirection;
use std::fmt;
use std::path::PathBuf;
use structopt::clap::AppSettings;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(bin_name = "cargo")]
pub enum Opts {
    #[structopt(
        name = "geiger",
        global_settings(&[
            AppSettings::UnifiedHelpMessage,
            AppSettings::DeriveDisplayOrder,
            AppSettings::DontCollapseArgsInUsage
        ])
    )]
    /// Detects usage of unsafe Rust in a Rust crate and its dependencies.
    Geiger(Args),
}

#[derive(StructOpt)]
pub struct Args {
    #[structopt(long = "package", short = "p", value_name = "SPEC")]
    /// Package to be used as the root of the tree
    pub package: Option<String>,

    #[structopt(long = "features", value_name = "FEATURES")]
    /// Space-separated list of features to activate
    pub features: Option<String>,

    #[structopt(long = "all-features")]
    /// Activate all available features
    pub all_features: bool,

    #[structopt(long = "no-default-features")]
    /// Do not activate the `default` feature
    pub no_default_features: bool,

    #[structopt(long = "target", value_name = "TARGET")]
    /// Set the target triple
    pub target: Option<String>,

    #[structopt(long = "all-targets")]
    /// Return dependencies for all targets. By default only the host target is matched.
    pub all_targets: bool,

    #[structopt(
        long = "manifest-path",
        value_name = "PATH",
        parse(from_os_str)
    )]
    /// Path to Cargo.toml
    pub manifest_path: Option<PathBuf>,

    #[structopt(long = "invert", short = "i")]
    /// Invert the tree direction
    pub invert: bool,

    #[structopt(long = "no-indent")]
    /// Display the dependencies as a list (rather than a tree)
    pub no_indent: bool,

    #[structopt(long = "prefix-depth")]
    /// Display the dependencies as a list (rather than a tree), but prefixed with the depth
    pub prefix_depth: bool,

    #[structopt(long = "all", short = "a")]
    /// Don't truncate dependencies that have already been displayed
    pub all: bool,

    #[structopt(
        long = "charset",
        value_name = "CHARSET",
        default_value = "utf8"
    )]
    /// Character set to use in output: utf8, ascii
    pub charset: Charset,

    #[structopt(
        long = "format",
        short = "f",
        value_name = "FORMAT",
        default_value = "{p}"
    )]
    /// Format string used for printing dependencies
    pub format: String,

    #[structopt(long = "verbose", short = "v", parse(from_occurrences))]
    /// Use verbose output (-vv very verbose/build.rs output)
    pub verbose: u32,

    #[structopt(long = "quiet", short = "q")]
    /// No output printed to stdout other than the tree
    pub quiet: Option<bool>,

    #[structopt(long = "color", value_name = "WHEN")]
    /// Coloring: auto, always, never
    pub color: Option<String>,

    #[structopt(long = "frozen")]
    /// Require Cargo.lock and cache are up to date
    pub frozen: bool,

    #[structopt(long = "locked")]
    /// Require Cargo.lock is up to date
    pub locked: bool,

    #[structopt(long = "offline")]
    /// Run without accessing the network
    pub offline: bool,

    #[structopt(short = "Z", value_name = "FLAG")]
    /// Unstable (nightly-only) flags to Cargo
    pub unstable_flags: Vec<String>,

    // TODO: Implement a new compact output mode where all metrics are
    // aggregated to a single used/unused ratio and output string.
    //#[structopt(long = "compact")]
    // Display compact output instead of table
    //compact: bool,
    #[structopt(long = "include-tests")]
    /// Count unsafe usage in tests.
    pub include_tests: bool,

    #[structopt(long = "build-dependencies", alias = "build-deps")]
    /// Also analyze build dependencies
    pub build_deps: bool,

    #[structopt(long = "dev-dependencies", alias = "dev-deps")]
    /// Also analyze dev dependencies
    pub dev_deps: bool,

    #[structopt(long = "all-dependencies", alias = "all-deps")]
    /// Analyze all dependencies, including build and dev
    pub all_deps: bool,

    #[structopt(long = "forbid-only")]
    /// Don't build or clean anything, only scan entry point .rs source files
    /// for forbid(unsafe_code) flags. This is significantly faster than the
    /// default scanning mode. TODO: Add ability to combine this with a
    /// whitelist for use in CI situations. Unsafe code in dependencies should
    /// not be able to sneak in undetected.
    pub forbid_only: bool,
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

    let target_dir = None; // Doesn't add any value for cargo-geiger.
    config.configure(
        args.verbose,
        args.quiet,
        &args.color,
        args.frozen,
        args.locked,
        args.offline,
        &target_dir,
        &args.unstable_flags,
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
        Some(args.target.as_ref().unwrap_or(&config_host).as_str())
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
        cfgs.as_ref().map(|r| &**r),
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
    let Opts::Geiger(args) = Opts::from_args();
    if let Err(e) = real_main(&args, &mut config) {
        let mut shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}
