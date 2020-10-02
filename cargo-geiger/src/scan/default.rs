use crate::args::Args;
use crate::format::emoji_symbols::EmojiSymbols;
use crate::format::print::OutputFormat;
use crate::format::table::{
    create_table_from_text_tree_lines, UNSAFE_COUNTERS_HEADER,
};
use crate::format::SymbolKind;
use crate::graph::Graph;
use crate::rs_file::resolve_rs_file_deps;
use crate::tree::traversal::walk_dependency_tree;

use super::find::find_unsafe;
use super::report::{ReportEntry, SafetyReport};
use super::{
    construct_rs_files_used_lines, list_files_used_but_not_scanned,
    package_metrics, unsafe_stats, ScanDetails, ScanMode, ScanParameters,
};

use cargo::core::compiler::CompileMode;
use cargo::core::shell::Verbosity;
use cargo::core::{PackageId, PackageSet, Workspace};
use cargo::ops::CompileOptions;
use cargo::{CliError, CliResult, Config};
use colored::Colorize;
use std::error::Error;
use std::fmt;

pub fn scan_unsafe(
    workspace: &Workspace,
    package_set: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    scan_parameters: &ScanParameters,
) -> CliResult {
    match scan_parameters.args.output_format {
        Some(format) => scan_to_report(
            workspace,
            package_set,
            root_pack_id,
            graph,
            scan_parameters,
            format,
        ),
        None => scan_to_table(
            workspace,
            package_set,
            root_pack_id,
            graph,
            scan_parameters,
        ),
    }
}

#[derive(Debug)]
struct FoundWarningsError {
    pub warning_count: u64,
}

impl Error for FoundWarningsError {}

/// Forward Display to Debug.
impl fmt::Display for FoundWarningsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Based on code from cargo-bloat. It seems weird that CompileOptions can be
/// constructed without providing all standard cargo options, TODO: Open an issue
/// in cargo?
fn build_compile_options<'a>(
    args: &'a Args,
    config: &'a Config,
) -> CompileOptions {
    let features = args
        .features
        .as_ref()
        .cloned()
        .unwrap_or_else(String::new)
        .split(' ')
        .map(str::to_owned)
        .collect::<Vec<String>>();
    let mut compile_options =
        CompileOptions::new(&config, CompileMode::Check { test: false })
            .unwrap();
    compile_options.features = features;
    compile_options.all_features = args.all_features;
    compile_options.no_default_features = args.no_default_features;

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

    compile_options
}

fn construct_scan_mode_default_output_key_lines(
    emoji_symbols: &EmojiSymbols,
) -> Vec<String> {
    let mut output_key_lines = Vec::<String>::new();

    output_key_lines.push(String::new());
    output_key_lines.push(String::from("Metric output format: x/y"));
    output_key_lines
        .push(String::from("    x = unsafe code used by the build"));
    output_key_lines
        .push(String::from("    y = total unsafe code found in the crate"));
    output_key_lines.push(String::new());

    output_key_lines.push(String::from("Symbols: "));
    let forbids = "No `unsafe` usage found, declares #![forbid(unsafe_code)]";
    let unknown = "No `unsafe` usage found, missing #![forbid(unsafe_code)]";
    let guilty = "`unsafe` usage found";

    let shift_sequence = if emoji_symbols.will_output_emoji() {
        "\r\x1B[7C" // The radiation icon's Unicode width is 2,
                    // but by most terminals it seems to be rendered at width 1.
    } else {
        ""
    };

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

    output_key_lines.push(format!(
        "    {: <2}{} = {}",
        emoji_symbols.emoji(SymbolKind::Rads),
        shift_sequence,
        guilty
    ));

    output_key_lines.push(String::new());

    output_key_lines.push(format!(
        "{}",
        UNSAFE_COUNTERS_HEADER
            .iter()
            .map(|s| s.to_owned())
            .collect::<Vec<_>>()
            .join(" ")
            .bold()
    ));

    output_key_lines.push(String::new());

    output_key_lines
}

fn scan(
    workspace: &Workspace,
    packages: &PackageSet,
    scan_parameters: &ScanParameters,
) -> Result<ScanDetails, CliError> {
    let compile_options =
        build_compile_options(scan_parameters.args, scan_parameters.config);
    let rs_files_used =
        resolve_rs_file_deps(&compile_options, workspace).unwrap();
    let geiger_context = find_unsafe(
        ScanMode::Full,
        scan_parameters.config,
        packages,
        scan_parameters.print_config,
    )?;
    Ok(ScanDetails {
        rs_files_used,
        geiger_context,
    })
}

fn scan_to_report(
    workspace: &Workspace,
    packages: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    scan_parameters: &ScanParameters,
    output_format: OutputFormat,
) -> CliResult {
    let ScanDetails {
        rs_files_used,
        geiger_context,
    } = scan(workspace, packages, scan_parameters)?;
    let mut report = SafetyReport::default();
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
        let unsafety = unsafe_stats(pack_metrics, &rs_files_used);
        let entry = ReportEntry { package, unsafety };
        report.packages.push(entry);
    }
    report.used_but_not_scanned_files =
        list_files_used_but_not_scanned(&geiger_context, &rs_files_used);
    let s = match output_format {
        OutputFormat::Json => serde_json::to_string(&report).unwrap(),
    };
    println!("{}", s);
    Ok(())
}

fn scan_to_table(
    workspace: &Workspace,
    package_set: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    scan_parameters: &ScanParameters,
) -> CliResult {
    let mut scan_output_lines = Vec::<String>::new();

    let ScanDetails {
        rs_files_used,
        geiger_context,
    } = scan(workspace, package_set, scan_parameters)?;

    if scan_parameters.print_config.verbosity == Verbosity::Verbose {
        let mut rs_files_used_lines =
            construct_rs_files_used_lines(&rs_files_used);
        scan_output_lines.append(&mut rs_files_used_lines);
    }

    let emoji_symbols = EmojiSymbols::new(scan_parameters.print_config.charset);
    let mut output_key_lines =
        construct_scan_mode_default_output_key_lines(&emoji_symbols);
    scan_output_lines.append(&mut output_key_lines);

    let text_tree_lines = walk_dependency_tree(
        root_pack_id,
        &graph,
        &scan_parameters.print_config,
    );
    let (mut table_lines, mut warning_count) =
        create_table_from_text_tree_lines(
            &geiger_context,
            package_set,
            scan_parameters.print_config,
            &rs_files_used,
            text_tree_lines,
        );
    scan_output_lines.append(&mut table_lines);

    for scan_output_line in scan_output_lines {
        println!("{}", scan_output_line);
    }

    let used_but_not_scanned =
        list_files_used_but_not_scanned(&geiger_context, &rs_files_used);
    warning_count += used_but_not_scanned.len() as u64;
    for path in &used_but_not_scanned {
        eprintln!(
            "WARNING: Dependency file was never scanned: {}",
            path.display()
        );
    }

    if warning_count > 0 {
        Err(CliError::new(
            anyhow::Error::new(FoundWarningsError { warning_count }),
            1,
        ))
    } else {
        Ok(())
    }
}

#[cfg(tests)]
mod default_tests {
    use super::*;
    use crate::format::Charset;

    #[test]
    fn build_compile_options_test() {
        let args_all_features = rand::random();
        let args_features = Some(String::from("unit test features"));
        let args_no_default_features = rand::random();

        let args = Args {
            all: false,
            all_deps: false,
            all_features: args_all_features,
            all_targets: false,
            build_deps: false,
            charset: Charset::Utf8,
            color: None,
            dev_deps: false,
            features: args_features,
            forbid_only: false,
            format: "".to_string(),
            frozen: false,
            help: false,
            include_tests: false,
            invert: false,
            locked: false,
            manifest_path: None,
            no_default_features: args_no_default_features,
            no_indent: false,
            offline: false,
            package: None,
            prefix_depth: false,
            quiet: false,
            target: None,
            unstable_flags: vec![],
            verbose: 0,
            version: false,
            output_format: None,
        };

        let config = Config::default().unwrap();

        let compile_options = build_compile_options(&args, &config);

        assert_eq!(compile_options.all_features, args_all_features);
        assert_eq!(compile_options.features, vec!["unit", "test", "features"]);
        assert_eq!(
            compile_options.no_default_features,
            args_no_default_features
        );
    }

    #[test]
    fn construct_scan_mode_default_output_key_lines_test() {
        let emoji_symbols = EmojiSymbols::new(Charset::Utf8);
        let output_key_lines =
            construct_scan_mode_default_output_key_lines(&emoji_symbols);

        assert_eq!(output_key_lines.len(), 12);
    }
}
