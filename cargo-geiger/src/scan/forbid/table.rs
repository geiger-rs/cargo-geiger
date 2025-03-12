use crate::format::emoji_symbols::EmojiSymbols;
use crate::format::pattern::Pattern;
use crate::format::print_config::PrintConfig;
use crate::format::{get_kind_group_name, SymbolKind};
use crate::graph::Graph;
use crate::mapping::CargoMetadataParameters;
use crate::scan::GeigerContext;
use crate::tree::traversal::walk_dependency_tree;
use crate::tree::TextTreeLine;

use super::super::find::find_unsafe;
use super::super::{ScanMode, ScanResult};

use cargo::{CliError, GlobalContext};
use colored::Colorize;
use krates::cm::PackageId;

pub fn scan_forbid_to_table(
    cargo_metadata_parameters: &CargoMetadataParameters,
    gctx: &GlobalContext,
    graph: &Graph,
    print_config: &PrintConfig,
    root_package_id: PackageId,
) -> Result<ScanResult, CliError> {
    let mut scan_output_lines = Vec::<String>::new();
    let emoji_symbols = EmojiSymbols::new(print_config.output_format);

    let mut output_key_lines = construct_key_lines(&emoji_symbols);
    scan_output_lines.append(&mut output_key_lines);

    let tree_lines = walk_dependency_tree(
        cargo_metadata_parameters,
        graph,
        print_config,
        root_package_id,
    );

    for tree_line in tree_lines {
        match tree_line {
            TextTreeLine::ExtraDepsGroup { kind, tree_vines } => {
                let name = get_kind_group_name(kind);
                if name.is_none() {
                    continue;
                }
                let name = name.unwrap();
                // TODO: Fix the alignment on macOS (others too?)
                scan_output_lines.push(format!("  {}{}", tree_vines, name));
            }
            TextTreeLine::Package {
                id: package_id,
                tree_vines,
            } => {
                let geiger_ctx = find_unsafe(
                    cargo_metadata_parameters,
                    gctx,
                    ScanMode::EntryPointsOnly,
                    print_config,
                )?;

                handle_package_text_tree_line(
                    cargo_metadata_parameters,
                    &emoji_symbols,
                    &geiger_ctx,
                    package_id,
                    print_config,
                    &mut scan_output_lines,
                    tree_vines,
                );
            }
        }
    }

    Ok(ScanResult {
        scan_output_lines,
        warning_count: 0,
    })
}

fn construct_key_lines(emoji_symbols: &EmojiSymbols) -> Vec<String> {
    let mut output_key_lines = vec![String::new(), String::from("Symbols:")];

    let forbids = "All entry point .rs files declare #![forbid(unsafe_code)].";
    let unknown = "This crate may use unsafe code.";

    let symbol_kinds_to_string_values = vec![
        (SymbolKind::Lock, forbids),
        (SymbolKind::QuestionMark, unknown),
    ];

    for (symbol_kind, string_values) in symbol_kinds_to_string_values {
        output_key_lines.push(format!(
            "    {: <2} = {}",
            emoji_symbols.emoji(symbol_kind),
            string_values
        ));
    }

    output_key_lines.push(String::new());
    output_key_lines
}

fn format_package_name(
    cargo_metadata_parameters: &CargoMetadataParameters,
    package_id: &PackageId,
    pattern: &Pattern,
) -> String {
    format!("{}", pattern.display(cargo_metadata_parameters, package_id))
}

fn handle_package_text_tree_line(
    cargo_metadata_parameters: &CargoMetadataParameters,
    emoji_symbols: &EmojiSymbols,
    geiger_ctx: &GeigerContext,
    package_id: PackageId,
    print_config: &PrintConfig,
    scan_output_lines: &mut Vec<String>,
    tree_vines: String,
) {
    let sym_lock = emoji_symbols.emoji(SymbolKind::Lock);
    let sym_qmark = emoji_symbols.emoji(SymbolKind::QuestionMark);

    let name = format_package_name(
        cargo_metadata_parameters,
        &package_id,
        &print_config.format,
    );
    let package_metrics = geiger_ctx.package_id_to_metrics.get(&package_id);
    let package_forbids_unsafe = match package_metrics {
        None => false, // no metrics available, .rs parsing failed?
        Some(package_metric) => package_metric.rs_path_to_metrics.iter().all(
            |(_k, rs_file_metrics_wrapper)| {
                rs_file_metrics_wrapper.metrics.forbids_unsafe
            },
        ),
    };
    let (symbol, name) = if package_forbids_unsafe {
        (&sym_lock, name.green())
    } else {
        (&sym_qmark, name.red())
    };
    scan_output_lines.push(format!("{} {}{}", symbol, tree_vines, name));
}

#[cfg(test)]
mod forbid_tests {
    use super::*;
    use crate::format::print_config::OutputFormat;
    use rstest::*;

    #[rstest]
    fn construct_scan_mode_forbid_only_output_key_lines_test() {
        let emoji_symbols = EmojiSymbols::new(OutputFormat::Utf8);
        let output_key_lines = construct_key_lines(&emoji_symbols);

        assert_eq!(output_key_lines.len(), 5);
    }
}
