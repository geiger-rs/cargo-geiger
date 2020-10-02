use crate::format::emoji_symbols::EmojiSymbols;
use crate::format::pattern::Pattern;
use crate::format::print::{OutputFormat, PrintConfig};
use crate::format::{get_kind_group_name, SymbolKind};
use crate::graph::Graph;
use crate::tree::traversal::walk_dependency_tree;
use crate::tree::TextTreeLine;

use super::find::find_unsafe;
use super::report::{QuickReportEntry, QuickSafetyReport};
use super::{package_metrics, ScanMode, ScanParameters};

use cargo::core::{Package, PackageId, PackageSet};
use cargo::{CliResult, Config};
use colored::Colorize;

pub fn scan_forbid_unsafe(
    packages: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    scan_parameters: &ScanParameters,
) -> CliResult {
    match scan_parameters.args.output_format {
        Some(format) => scan_forbid_to_report(
            scan_parameters.config,
            packages,
            root_pack_id,
            graph,
            scan_parameters.print_config,
            format,
        ),
        None => scan_forbid_to_table(
            scan_parameters.config,
            packages,
            root_pack_id,
            graph,
            scan_parameters.print_config,
        ),
    }
}

fn construct_scan_mode_forbid_only_output_key_lines(
    emoji_symbols: &EmojiSymbols,
) -> Vec<String> {
    let mut output_key_lines = Vec::<String>::new();

    output_key_lines.push(String::new());
    output_key_lines.push(String::from("Symbols: "));

    let forbids = "All entry point .rs files declare #![forbid(unsafe_code)].";
    let unknown = "This crate may use unsafe code.";

    output_key_lines.push(format!(
        "    {: <2} = {}",
        emoji_symbols.emoji(SymbolKind::Lock),
        forbids
    ));

    output_key_lines.push(format!(
        "    {: <2} = {}",
        emoji_symbols.emoji(SymbolKind::QuestionMark),
        unknown
    ));

    output_key_lines.push(String::new());

    output_key_lines
}

fn format_package_name(package: &Package, pattern: &Pattern) -> String {
    format!(
        "{}",
        pattern.display(&package.package_id(), package.manifest().metadata())
    )
}

pub fn scan_forbid_to_table(
    config: &Config,
    packages: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    print_config: &PrintConfig,
) -> CliResult {
    let mut scan_output_lines = Vec::<String>::new();

    let emoji_symbols = EmojiSymbols::new(print_config.charset);
    let sym_lock = emoji_symbols.emoji(SymbolKind::Lock);
    let sym_qmark = emoji_symbols.emoji(SymbolKind::QuestionMark);

    let geiger_ctx =
        find_unsafe(ScanMode::EntryPointsOnly, config, packages, print_config)?;

    let mut output_key_lines =
        construct_scan_mode_forbid_only_output_key_lines(&emoji_symbols);

    scan_output_lines.append(&mut output_key_lines);

    let tree_lines = walk_dependency_tree(root_pack_id, &graph, &print_config);
    for tree_line in tree_lines {
        match tree_line {
            TextTreeLine::Package { id, tree_vines } => {
                let package = packages.get_one(id).unwrap(); // FIXME
                let name = format_package_name(package, print_config.format);
                let pack_metrics = geiger_ctx.pack_id_to_metrics.get(&id);
                let package_forbids_unsafe = match pack_metrics {
                    None => false, // no metrics available, .rs parsing failed?
                    Some(pm) => pm
                        .rs_path_to_metrics
                        .iter()
                        .all(|(_k, v)| v.metrics.forbids_unsafe),
                };
                let (symbol, name) = if package_forbids_unsafe {
                    (&sym_lock, name.green())
                } else {
                    (&sym_qmark, name.red())
                };
                scan_output_lines
                    .push(format!("{} {}{}", symbol, tree_vines, name));
            }
            TextTreeLine::ExtraDepsGroup { kind, tree_vines } => {
                let name = get_kind_group_name(kind);
                if name.is_none() {
                    continue;
                }
                let name = name.unwrap();
                // TODO: Fix the alignment on macOS (others too?)
                scan_output_lines.push(format!("  {}{}", tree_vines, name));
            }
        }
    }

    for scan_output_line in scan_output_lines {
        println!("{}", scan_output_line);
    }

    Ok(())
}

fn scan_forbid_to_report(
    config: &Config,
    packages: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    print_config: &PrintConfig,
    output_format: OutputFormat,
) -> CliResult {
    let geiger_context =
        find_unsafe(ScanMode::EntryPointsOnly, config, packages, print_config)?;
    let mut report = QuickSafetyReport::default();
    for (package, pack_metrics) in
        package_metrics(&geiger_context, graph, root_pack_id)
    {
        let pack_metrics = match pack_metrics {
            Some(m) => m,
            None => {
                report.packages_without_metrics.push(package.id);
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
        report.packages.push(entry);
    }
    let s = match output_format {
        OutputFormat::Json => serde_json::to_string(&report).unwrap(),
    };
    println!("{}", s);
    Ok(())
}

#[cfg(test)]
mod forbid_tests {
    use super::*;

    use crate::format::Charset;

    use cargo::core::Workspace;
    use cargo::util::important_paths;

    #[test]
    fn construct_scan_mode_forbid_only_output_key_lines_test() {
        let emoji_symbols = EmojiSymbols::new(Charset::Utf8);
        let output_key_lines =
            construct_scan_mode_forbid_only_output_key_lines(&emoji_symbols);

        assert_eq!(output_key_lines.len(), 5);
    }

    #[test]
    fn format_package_name_test() {
        let pattern = Pattern::try_build("{p}").unwrap();

        let config = Config::default().unwrap();
        let workspace = Workspace::new(
            &important_paths::find_root_manifest_for_wd(config.cwd()).unwrap(),
            &config,
        )
        .unwrap();

        let package = workspace.current().unwrap();

        let formatted_package_name = format_package_name(&package, &pattern);

        assert_eq!(formatted_package_name, "cargo-geiger 0.10.2");
    }
}
