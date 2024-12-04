//! The outer CLI parts of the `cargo-geiger` cargo plugin executable.
//! TODO: Refactor this file to only deal with command line argument processing.

#![deny(clippy::cargo)]
#![deny(clippy::doc_markdown)]
#![forbid(unsafe_code)]

extern crate cargo;
extern crate colored;
extern crate petgraph;
extern crate strum;
extern crate strum_macros;

use cargo_geiger::args::{Args, HELP};
use cargo_geiger::cli::{get_cargo_metadata, get_krates, get_workspace};
use cargo_geiger::graph::build_graph;
use cargo_geiger::mapping::{CargoMetadataParameters, QueryResolve};
use cargo_geiger::readme::create_or_replace_section_in_readme;
use cargo_geiger::scan::{scan, FoundWarningsError, ScanResult};

use cargo::core::shell::Shell;
use cargo::util::important_paths;
use cargo::{CliError, CliResult, Config};

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

fn cli_result_main(args: &Args) -> CliResult {
    if args.version {
        println!("cargo-geiger {}", VERSION.unwrap_or("unknown version"));
        return Ok(());
    }
    if args.help {
        println!("{}", HELP);
        return Ok(());
    }

    let mut config = Config::default()?;
    args.update_config(&mut config)?;

    let cargo_metadata = get_cargo_metadata(args, &config)?;
    let krates = get_krates(&cargo_metadata)?;

    let cargo_metadata_parameters = CargoMetadataParameters {
        metadata: &cargo_metadata,
        krates: &krates,
    };

    let workspace = get_workspace(&config, args.manifest_path.clone())?;

    let cargo_metadata_root_package_id = if let Some(
        cargo_metadata_root_package,
    ) = cargo_metadata.root_package()
    {
        cargo_metadata_root_package.id.clone()
    } else {
        eprintln!(
            "manifest path `{}` is a virtual manifest, but this command requires running against an actual package in this workspace",
            match args.manifest_path.clone() {
                Some(path) => path,
                None => important_paths::find_root_manifest_for_wd(config.cwd())?,
            }.as_os_str().to_str().unwrap()
        );

        return Err(CliError::code(1));
    };

    let global_rustc = config.load_global_rustc(Some(&workspace))?;

    let graph = build_graph(
        args,
        &cargo_metadata_parameters,
        &global_rustc.host,
        &global_rustc.path,
        cargo_metadata_root_package_id.clone(),
    )?;

    let query_resolve_root_package_id = args.package.as_ref().map_or(
        cargo_metadata_root_package_id.clone(),
        |package_query| {
            krates
                .query_resolve(package_query)
                .map_or(cargo_metadata_root_package_id, |package_id| package_id)
        },
    );

    let ScanResult {
        scan_output_lines,
        warning_count,
    } = scan(
        args,
        &cargo_metadata_parameters,
        &config,
        &graph,
        query_resolve_root_package_id,
        &workspace,
    )?;

    if args.readme_args.update_readme {
        create_or_replace_section_in_readme(
            &args.readme_args,
            &scan_output_lines,
        )?;
    } else {
        for scan_output_line in scan_output_lines {
            println!("{}", scan_output_line);
        }
    }

    if warning_count > 0 {
        return Err(CliError::new(
            anyhow::Error::new(FoundWarningsError { warning_count }),
            1,
        ));
    }

    Ok(())
}

fn main() {
    let args = Args::parse_args(pico_args::Arguments::from_env()).unwrap();
    if let Err(e) = cli_result_main(&args) {
        let mut shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}
