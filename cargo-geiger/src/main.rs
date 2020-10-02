//! The outer CLI parts of the `cargo-geiger` cargo plugin executable.
//! TODO: Refactor this file to only deal with command line argument processing.

#![forbid(unsafe_code)]
#![forbid(warnings)]

extern crate cargo;
extern crate colored;
extern crate petgraph;
extern crate strum;
extern crate strum_macros;

mod args;
mod cli;
mod format;
mod graph;
mod rs_file;
mod scan;
mod tree;

use crate::args::{Args, HELP};
use crate::cli::{get_cfgs, get_registry, get_workspace, resolve};
use crate::format::pattern::Pattern;
use crate::format::print::{Prefix, PrintConfig};
use crate::format::FormatError;
use crate::graph::{build_graph, ExtraDeps};
use crate::scan::default::scan_unsafe;
use crate::scan::forbid::scan_forbid_unsafe;
use crate::scan::ScanParameters;

use cargo::core::shell::{ColorChoice, Shell, Verbosity};
use cargo::util::errors::CliError;
use cargo::{CliResult, Config};
use geiger::IncludeTests;
use petgraph::EdgeDirection;

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

fn real_main(args: &Args, config: &mut Config) -> CliResult {
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
        args.quiet,
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

    let workspace = get_workspace(config, args.manifest_path.clone())?;
    let package = workspace.current()?;
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
        &workspace,
        &features,
        args.all_features,
        args.no_default_features,
    )?;
    let package_ids = packages.package_ids().collect::<Vec<_>>();
    let packages = registry.get(&package_ids)?;

    let root_pack_id = match args.package {
        Some(ref pkg) => resolve.query(pkg)?,
        None => package.package_id(),
    };

    let config_host = config.load_global_rustc(Some(&workspace))?.host;
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

    let cfgs = get_cfgs(config, &args.target, &workspace)?;
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
    let print_config = PrintConfig {
        all: args.all,
        verbosity,
        direction,
        prefix,
        format: &format,
        charset: args.charset,
        allow_partial_results,
        include_tests,
        output_format: args.output_format,
    };

    let scan_parameters = ScanParameters {
        args: &args,
        config: &config,
        print_config: &print_config,
    };

    if args.forbid_only {
        scan_forbid_unsafe(&packages, root_pack_id, &graph, &scan_parameters)
    } else {
        scan_unsafe(
            &workspace,
            &packages,
            root_pack_id,
            &graph,
            &scan_parameters,
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
    let args = Args::parse_args().unwrap();
    if let Err(e) = real_main(&args, &mut config) {
        let mut shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}
