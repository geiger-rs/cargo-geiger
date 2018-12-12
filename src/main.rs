extern crate cargo;
extern crate cargo_geiger;
extern crate colored;
extern crate petgraph;
extern crate structopt;

use cargo::core::shell::Shell;
use cargo::core::shell::Verbosity;
use cargo::{CliResult, Config};
use cargo_geiger::format::Pattern;
use cargo_geiger::*;
use colored::*;
use petgraph::EdgeDirection;
use std::path::PathBuf;
use structopt::StructOpt;

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
    let packages = registry.get(&ids);

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

    let format = Pattern::new(&args.format)
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

    let mut rs_files_used = resolve_rs_file_deps(&args, &ws).unwrap();

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
