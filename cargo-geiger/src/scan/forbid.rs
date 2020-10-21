mod table;

use crate::format::print_config::{OutputFormat, PrintConfig};
use crate::graph::Graph;

use super::find::find_unsafe;
use super::{package_metrics, ScanMode, ScanParameters};

use table::scan_forbid_to_table;

use cargo::core::{PackageId, PackageSet};
use cargo::{CliResult, Config};
use cargo_geiger_serde::{QuickReportEntry, QuickSafetyReport};

pub fn scan_forbid_unsafe(
    package_set: &PackageSet,
    root_package_id: PackageId,
    graph: &Graph,
    scan_parameters: &ScanParameters,
) -> CliResult {
    match scan_parameters.args.output_format {
        Some(output_format) => scan_forbid_to_report(
            scan_parameters.config,
            package_set,
            root_package_id,
            graph,
            scan_parameters.print_config,
            output_format,
        ),
        None => scan_forbid_to_table(
            scan_parameters.config,
            package_set,
            root_package_id,
            graph,
            scan_parameters.print_config,
        ),
    }
}

fn scan_forbid_to_report(
    config: &Config,
    packages: &PackageSet,
    root_package_id: PackageId,
    graph: &Graph,
    print_config: &PrintConfig,
    output_format: OutputFormat,
) -> CliResult {
    let geiger_context =
        find_unsafe(ScanMode::EntryPointsOnly, config, packages, print_config)?;
    let mut report = QuickSafetyReport::default();
    for (package, package_metrics) in
        package_metrics(&geiger_context, graph, root_package_id)
    {
        let pack_metrics = match package_metrics {
            Some(m) => m,
            None => {
                report.packages_without_metrics.insert(package.id);
                continue;
            }
        };
        let forbids_unsafe = pack_metrics.rs_path_to_metrics.iter().all(
            |(_, rs_file_metrics_wrapper)| {
                rs_file_metrics_wrapper.metrics.forbids_unsafe
            },
        );
        let entry = QuickReportEntry {
            package,
            forbids_unsafe,
        };
        report.packages.insert(entry.package.id.clone(), entry);
    }
    let s = match output_format {
        OutputFormat::Json => serde_json::to_string(&report).unwrap(),
    };
    println!("{}", s);
    Ok(())
}
