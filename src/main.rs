extern crate cargo;
extern crate cargo_geiger;
extern crate colored;
extern crate petgraph;
extern crate structopt;

use cargo::core::compiler::CompileMode;
use cargo::core::resolver::Method;
use cargo::core::shell::Shell;
use cargo::core::shell::Verbosity;
use cargo::ops::CompileOptions;
use cargo::{CliResult, Config};
use cargo_geiger::format::Pattern;
use cargo_geiger::resolve;
use cargo_geiger::print_tree;
use cargo_geiger::PrintConfig;
use cargo_geiger::UNSAFE_COUNTERS_HEADER; 
use cargo_geiger::resolve_rs_file_deps;
use cargo_geiger::UTF8_SYMBOLS;
use cargo_geiger::ASCII_SYMBOLS; 
use cargo_geiger::build_graph;
use cargo_geiger::Charset;
use cargo_geiger::workspace;
use cargo_geiger::registry;
use cargo_geiger::get_cfgs;
use cargo_geiger::Prefix;
use cargo_geiger::IncludeTests;
use colored::*;
use petgraph::EdgeDirection;
use std::path::PathBuf;
use structopt::clap::AppSettings;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(bin_name = "cargo")]
pub enum Opts {
    #[structopt(
        name = "geiger",
        raw(
            setting = "AppSettings::UnifiedHelpMessage",
            setting = "AppSettings::DeriveDisplayOrder",
            setting = "AppSettings::DontCollapseArgsInUsage"
        )
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
}

/// Based on code from cargo-bloat. It seems weird that CompileOptions can be
/// constructed without providing all standard cargo options, TODO: Open an issue
/// in cargo?
pub fn build_compile_options<'a>(
    args: &'a Args,
    config: &'a Config,
) -> CompileOptions<'a> {
    let features = Method::split_features(
        &args.features.clone().into_iter().collect::<Vec<_>>(),
    )
    .into_iter()
    .map(|s| s.to_string());
    let mut opt =
        CompileOptions::new(&config, CompileMode::Check { test: false })
            .unwrap();
    opt.features = features.collect::<_>();
    opt.all_features = args.all_features;
    opt.no_default_features = args.no_default_features;

    // TODO: Investigate if this is relevant to cargo-geiger.
    //let mut bins = Vec::new();
    //let mut examples = Vec::new();
    // opt.release = args.release;
    // opt.target = args.target.clone();
    // if let Some(ref name) = args.bin {
    //     bins.push(name.clone());
    // } else if let Some(ref name) = args.example {
    //     examples.push(name.clone());
    // }
    // if args.bin.is_some() || args.example.is_some() {
    //     opt.filter = ops::CompileFilter::new(
    //         false,
    //         bins.clone(), false,
    //         Vec::new(), false,
    //         examples.clone(), false,
    //         Vec::new(), false,
    //         false,
    //     );
    // }

    opt
}

fn real_main(args: &Args, config: &mut Config) -> CliResult {
    let target_dir = None; // Doesn't add any value for cargo-geiger.
    config.configure(
        args.verbose,
        args.quiet,
        &args.color,
        args.frozen,
        args.locked,
        &target_dir,
        &args.unstable_flags,
    )?;
    let verbosity = if args.verbose == 0 {
        Verbosity::Normal
    } else {
        Verbosity::Verbose
    };
    let ws = workspace(config, args.manifest_path.clone())?;
    let package = ws.current()?;
    let mut registry = registry(config, &package)?;
    let (packages, resolve) = resolve(
        &mut registry,
        &ws,
        args.features.clone(),
        args.all_features,
        args.no_default_features,
    )?;
    let ids = packages.package_ids().cloned().collect::<Vec<_>>();
    let packages = registry.get(&ids)?;

    let root = match args.package {
        Some(ref pkg) => resolve.query(pkg)?,
        None => package.package_id(),
    };

    let config_host = config.rustc(Some(&ws))?.host;
    let target = if args.all_targets {
        None
    } else {
        Some(args.target.as_ref().unwrap_or(&config_host).as_str())
    };

    let format = Pattern::try_build(&args.format)
        .map_err(|e| failure::err_msg(e.to_string()))?;

    let cfgs = get_cfgs(config, &args.target, &ws)?;
    let graph = build_graph(
        &resolve,
        &packages,
        package.package_id(),
        target,
        cfgs.as_ref().map(|r| &**r),
    )?;

    let direction = if args.invert {
        EdgeDirection::Incoming
    } else {
        EdgeDirection::Outgoing
    };

    let symbols = match args.charset {
        Charset::Ascii => &ASCII_SYMBOLS,
        Charset::Utf8 => &UTF8_SYMBOLS,
    };

    let prefix = if args.prefix_depth {
        Prefix::Depth
    } else if args.no_indent {
        Prefix::None
    } else {
        Prefix::Indent
    };

    let copt = build_compile_options(args, config);
    let mut rs_files_used = resolve_rs_file_deps(&copt, &ws).unwrap();

    if verbosity == Verbosity::Verbose {
        // Print all .rs files found through the .d files, in sorted order.
        let mut paths = rs_files_used
            .keys()
            .map(|k| k.to_owned())
            .collect::<Vec<PathBuf>>();
        paths.sort();
        paths
            .iter()
            .for_each(|p| println!("Used by build (sorted): {}", p.display()));
    }

    println!();
    println!("Metric output format: x/y");
    println!("x = unsafe code used by the build");
    println!("y = total unsafe code found in the crate");
    println!();
    println!(
        "{}",
        UNSAFE_COUNTERS_HEADER
            .iter()
            .map(|s| s.to_owned())
            .collect::<Vec<_>>()
            .join(" ")
            .bold()
    );
    println!();

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
        symbols,
        allow_partial_results,
        include_tests,
    };
    print_tree(root, &graph, &mut rs_files_used, &pc);
    rs_files_used
        .iter()
        .filter(|(_k, v)| **v == 0)
        .for_each(|(k, _v)| {
            println!(
                "WARNING: Dependency file was never scanned: {}",
                k.display()
            )
        });
    Ok(())
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
