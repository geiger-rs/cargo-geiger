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
mod mapping;
mod readme;
mod scan;
mod tree;

use crate::args::{Args, HELP};
use crate::cli::{get_cargo_metadata, get_krates, get_workspace};
use crate::graph::build_graph;
use crate::mapping::{CargoMetadataParameters, QueryResolve};
use crate::scan::scan;

use cargo::core::shell::Shell;
use cargo::util::important_paths;
use cargo::{CliError, CliResult, Config};
use readme::create_or_replace_section_in_readme;

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

    args.update_config(config)?;

    let cargo_metadata = get_cargo_metadata(&args, config)?;
    let krates = get_krates(&cargo_metadata)?;

    let cargo_metadata_parameters = CargoMetadataParameters {
        metadata: &cargo_metadata,
        krates: &krates,
    };

    let workspace = get_workspace(config, args.manifest_path.clone())?;

    let cargo_metadata_root_package_id;

    if let Some(cargo_metadata_root_package) = cargo_metadata.root_package() {
        cargo_metadata_root_package_id = cargo_metadata_root_package.id.clone();
    } else {
        eprintln!(
            "manifest path `{}` is a virtual manifest, but this command requires running against an actual package in this workspace",
            match args.manifest_path.clone() {
                Some(path) => path,
                None => important_paths::find_root_manifest_for_wd(config.cwd())?,
            }.as_os_str().to_str().unwrap()
        );

        return CliResult::Err(CliError::code(1));
    }

    let graph = build_graph(
        args,
        &cargo_metadata_parameters,
        config,
        cargo_metadata_root_package_id.clone(),
        &workspace,
    )?;

    let query_resolve_root_package_id = args.package.as_ref().map_or(
        cargo_metadata_root_package_id.clone(),
        |ref package_query| {
            krates
                .query_resolve(package_query)
                .map_or(cargo_metadata_root_package_id, |package_id| package_id)
        },
    );

    let scan_output_lines = scan(
        args,
        &cargo_metadata_parameters,
        config,
        &graph,
        query_resolve_root_package_id,
        &workspace,
    )?;

    if args.update_readme {
        let mut current_dir_path_buf = std::env::current_dir().unwrap();
        current_dir_path_buf.push(readme::README_FILENAME);

        create_or_replace_section_in_readme(
            current_dir_path_buf,
            &scan_output_lines,
        )?;
    } else {
        for scan_output_line in scan_output_lines {
            println!("{}", scan_output_line);
        }
    }

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
    let args = Args::parse_args(pico_args::Arguments::from_env()).unwrap();
    if let Err(e) = real_main(&args, &mut config) {
        let mut shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}
