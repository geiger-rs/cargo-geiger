use crate::find::find_unsafe_in_packages;
use crate::format::print::PrintConfig;
use crate::format::table::{
    print_text_tree_lines_as_table, UNSAFE_COUNTERS_HEADER,
};
use crate::format::tree::TextTreeLine;
use crate::format::{get_kind_group_name, EmojiSymbols, Pattern, SymbolKind};
use crate::graph::Graph;
use crate::rs_file::resolve_rs_file_deps;
use crate::traversal::walk_dependency_tree;
use crate::Args;

use cargo::core::compiler::CompileMode;
use cargo::core::package::PackageSet;
use cargo::core::shell::Verbosity;
use cargo::core::{Package, PackageId, Workspace};
use cargo::ops::CompileOptions;
use cargo::util::CargoResult;
use cargo::Config;
use cargo::{CliError, CliResult};
use colored::Colorize;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

pub enum ScanMode {
    // The default scan mode, scan every .rs file.
    Full,

    // An optimization to allow skipping everything except the entry points.
    // This is only useful for the "--forbid-only" mode since that mode only
    // depends on entry point .rs files.
    EntryPointsOnly,
}

pub fn run_scan_mode_default(
    config: &Config,
    ws: &Workspace,
    packages: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    pc: &PrintConfig,
    args: &Args,
) -> CliResult {
    let copt = build_compile_options(args, config);
    let rs_files_used = resolve_rs_file_deps(&copt, &ws).unwrap();
    if pc.verbosity == Verbosity::Verbose {
        // Print all .rs files found through the .d files, in sorted order.
        let mut paths = rs_files_used
            .iter()
            .map(std::borrow::ToOwned::to_owned)
            .collect::<Vec<PathBuf>>();
        paths.sort();
        paths
            .iter()
            .for_each(|p| println!("Used by build (sorted): {}", p.display()));
    }
    let mut progress = cargo::util::Progress::new("Scanning", config);
    let emoji_symbols = EmojiSymbols::new(pc.charset);
    let geiger_ctx = find_unsafe_in_packages(
        &packages,
        pc.allow_partial_results,
        pc.include_tests,
        ScanMode::Full,
        |i, count| -> CargoResult<()> { progress.tick(i, count) },
    );
    progress.clear();
    config.shell().status("Scanning", "done")?;

    println!();
    println!("Metric output format: x/y");
    println!("    x = unsafe code used by the build");
    println!("    y = total unsafe code found in the crate");
    println!();

    println!("Symbols: ");
    let forbids = "No `unsafe` usage found, declares #![forbid(unsafe_code)]";
    let unknown = "No `unsafe` usage found, missing #![forbid(unsafe_code)]";
    let guilty = "`unsafe` usage found";

    let shift_sequence = if emoji_symbols.will_output_emoji() {
        "\r\x1B[7C" // The radiation icon's Unicode width is 2,
                    // but by most terminals it seems to be rendered at width 1.
    } else {
        ""
    };

    println!(
        "    {: <2} = {}",
        emoji_symbols.emoji(SymbolKind::Lock),
        forbids
    );
    println!(
        "    {: <2} = {}",
        emoji_symbols.emoji(SymbolKind::QuestionMark),
        unknown
    );
    println!(
        "    {: <2}{} = {}",
        emoji_symbols.emoji(SymbolKind::Rads),
        shift_sequence,
        guilty
    );
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

    let tree_lines = walk_dependency_tree(root_pack_id, &graph, &pc);
    let mut warning_count = print_text_tree_lines_as_table(
        &geiger_ctx,
        packages,
        pc,
        &rs_files_used,
        tree_lines,
    );

    println!();
    let scanned_files = geiger_ctx
        .pack_id_to_metrics
        .iter()
        .flat_map(|(_k, v)| v.rs_path_to_metrics.keys())
        .collect::<HashSet<&PathBuf>>();
    let used_but_not_scanned =
        rs_files_used.iter().filter(|p| !scanned_files.contains(p));
    for path in used_but_not_scanned {
        eprintln!(
            "WARNING: Dependency file was never scanned: {}",
            path.display()
        );
        warning_count += 1;
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

pub fn run_scan_mode_forbid_only(
    config: &Config,
    packages: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    pc: &PrintConfig,
) -> CliResult {
    let emoji_symbols = EmojiSymbols::new(pc.charset);
    let mut progress = cargo::util::Progress::new("Scanning", config);
    let geiger_ctx = find_unsafe_in_packages(
        &packages,
        pc.allow_partial_results,
        pc.include_tests,
        ScanMode::EntryPointsOnly,
        |i, count| -> CargoResult<()> { progress.tick(i, count) },
    );
    progress.clear();
    config.shell().status("Scanning", "done")?;

    println!();

    println!("Symbols: ");
    let forbids = "All entry point .rs files declare #![forbid(unsafe_code)].";
    let unknown = "This crate may use unsafe code.";

    let sym_lock = emoji_symbols.emoji(SymbolKind::Lock);
    let sym_qmark = emoji_symbols.emoji(SymbolKind::QuestionMark);

    println!("    {: <2} = {}", sym_lock, forbids);
    println!("    {: <2} = {}", sym_qmark, unknown);
    println!();

    let tree_lines = walk_dependency_tree(root_pack_id, &graph, &pc);
    for tl in tree_lines {
        match tl {
            TextTreeLine::Package { id, tree_vines } => {
                let pack = packages.get_one(id).unwrap(); // FIXME
                let name = format_package_name(pack, pc.format);
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
                println!("{} {}{}", symbol, tree_vines, name);
            }
            TextTreeLine::ExtraDepsGroup { kind, tree_vines } => {
                let name = get_kind_group_name(kind);
                if name.is_none() {
                    continue;
                }
                let name = name.unwrap();
                // TODO: Fix the alignment on macOS (others too?)
                println!("  {}{}", tree_vines, name);
            }
        }
    }

    Ok(())
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
    let mut opt =
        CompileOptions::new(&config, CompileMode::Check { test: false })
            .unwrap();
    opt.features = features;
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

fn format_package_name(pack: &Package, pat: &Pattern) -> String {
    format!(
        "{}",
        pat.display(&pack.package_id(), pack.manifest().metadata())
    )
}
