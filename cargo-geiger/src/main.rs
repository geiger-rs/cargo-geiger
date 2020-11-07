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
mod krates_utils;
mod scan;
mod tree;

use crate::args::{Args, HELP};
use crate::cli::{
    get_cargo_metadata, get_krates, get_registry, get_workspace, resolve,
};
use crate::graph::build_graph;
use crate::scan::scan;

use crate::krates_utils::{CargoMetadataParameters, ToCargoMetadataPackage};
use cargo::core::shell::{ColorChoice, Shell};
use cargo::{CliResult, Config};

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

    match config.shell().color_choice() {
        ColorChoice::Always => colored::control::set_override(true),
        ColorChoice::Never => colored::control::set_override(false),
        ColorChoice::CargoAuto => {}
    }

    let cargo_metadata = get_cargo_metadata(&args, config)?;
    let krates = get_krates(&cargo_metadata)?;

    let cargo_metadata_parameters = CargoMetadataParameters {
        metadata: &cargo_metadata,
        krates: &krates,
    };

    let workspace = get_workspace(config, args.manifest_path.clone())?;
    let root_package = workspace.current()?;
    let mut registry = get_registry(config, &root_package)?;

    // `cargo_metadata.root_package()` will return `None` when called on a virtual
    // manifest
    let cargo_metadata_root_package_id = root_package
        .to_cargo_metadata_package(cargo_metadata_parameters.metadata)
        .id;

    let (package_set, resolve) = resolve(
        &args.features_args,
        root_package.package_id(),
        &mut registry,
        &workspace,
    )?;

    let package_ids = package_set.package_ids().collect::<Vec<_>>();
    let package_set = registry.get(&package_ids)?;

    let root_package_id = match args.package {
        Some(ref pkg) => resolve.query(pkg)?,
        None => root_package.package_id(),
    };

    let graph = build_graph(
        args,
        &cargo_metadata_parameters,
        config,
        &resolve,
        &package_set,
        cargo_metadata_root_package_id,
        &workspace,
    )?;

    scan(
        args,
        &cargo_metadata_parameters,
        config,
        &graph,
        &package_set,
        root_package_id,
        &workspace,
    )
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
    let args = Args::parse_args(pico_args::Arguments::from_env()).unwrap();
    if let Err(e) = real_main(&args, &mut config) {
        let mut shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}
