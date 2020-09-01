use crate::find::GeigerContext;
use crate::format::print::{colorize, PrintConfig};
use crate::format::tree::TextTreeLine;
use crate::format::{get_kind_group_name, EmojiSymbols, SymbolKind};
use crate::rs_file::PackageMetrics;

use cargo::core::package::PackageSet;
use geiger::{Count, CounterBlock, DetectionStatus};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

// ---------- BEGIN: Public items ----------

// TODO: use a table library, or factor the tableness out in a smarter way. This
// is probably easier now when the tree formatting is separated from the tree
// traversal.
pub const UNSAFE_COUNTERS_HEADER: [&str; 6] = [
    "Functions ",
    "Expressions ",
    "Impls ",
    "Traits ",
    "Methods ",
    "Dependency",
];

pub fn print_text_tree_lines_as_table(
    geiger_context: &GeigerContext,
    package_set: &PackageSet,
    print_config: &PrintConfig,
    rs_files_used: &HashSet<PathBuf>,
    text_tree_lines: Vec<TextTreeLine>,
) -> u64 {
    let mut total_packs_none_detected_forbids_unsafe = 0;
    let mut total_packs_none_detected_allows_unsafe = 0;
    let mut total_packs_unsafe_detected = 0;
    let mut package_status = HashMap::new();
    let mut total = CounterBlock::default();
    let mut total_unused = CounterBlock::default();
    let mut warning_count = 0;

    for tl in text_tree_lines {
        match tl {
            TextTreeLine::Package { id, tree_vines } => {
                let pack = package_set.get_one(id).unwrap_or_else(|_| {
                    // TODO: Avoid panic, return Result.
                    panic!("Expected to find package by id: {}", id);
                });
                let pack_metrics =
                    match geiger_context.pack_id_to_metrics.get(&id) {
                        Some(m) => m,
                        None => {
                            eprintln!(
                                "WARNING: No metrics found for package: {}",
                                id
                            );
                            warning_count += 1;
                            continue;
                        }
                    };
                package_status.entry(id).or_insert_with(|| {
                    let unsafe_found = pack_metrics
                        .rs_path_to_metrics
                        .iter()
                        .filter(|(k, _)| rs_files_used.contains(k.as_path()))
                        .any(|(_, v)| v.metrics.counters.has_unsafe());

                    // The crate level "forbids unsafe code" metric __used to__ only
                    // depend on entry point source files that were __used by the
                    // build__. This was too subtle in my opinion. For a crate to be
                    // classified as forbidding unsafe code, all entry point source
                    // files must declare `forbid(unsafe_code)`. Either a crate
                    // forbids all unsafe code or it allows it _to some degree_.
                    let crate_forbids_unsafe = pack_metrics
                        .rs_path_to_metrics
                        .iter()
                        .filter(|(_, v)| v.is_crate_entry_point)
                        .all(|(_, v)| v.metrics.forbids_unsafe);

                    for (k, v) in &pack_metrics.rs_path_to_metrics {
                        //println!("{}", k.display());
                        let target = if rs_files_used.contains(k) {
                            &mut total
                        } else {
                            &mut total_unused
                        };
                        *target = target.clone() + v.metrics.counters.clone();
                    }
                    match (unsafe_found, crate_forbids_unsafe) {
                        (false, true) => {
                            total_packs_none_detected_forbids_unsafe += 1;
                            DetectionStatus::NoneDetectedForbidsUnsafe
                        }
                        (false, false) => {
                            total_packs_none_detected_allows_unsafe += 1;
                            DetectionStatus::NoneDetectedAllowsUnsafe
                        }
                        (true, _) => {
                            total_packs_unsafe_detected += 1;
                            DetectionStatus::UnsafeDetected
                        }
                    }
                });
                let emoji_symbols = EmojiSymbols::new(print_config.charset);
                let detection_status =
                    package_status.get(&id).unwrap_or_else(|| {
                        panic!("Expected to find package by id: {}", &id)
                    });
                let icon = match detection_status {
                    DetectionStatus::NoneDetectedForbidsUnsafe => {
                        emoji_symbols.emoji(SymbolKind::Lock)
                    }
                    DetectionStatus::NoneDetectedAllowsUnsafe => {
                        emoji_symbols.emoji(SymbolKind::QuestionMark)
                    }
                    DetectionStatus::UnsafeDetected => {
                        emoji_symbols.emoji(SymbolKind::Rads)
                    }
                };
                let pack_name = colorize(
                    format!(
                        "{}",
                        print_config
                            .format
                            .display(&id, pack.manifest().metadata())
                    ),
                    &detection_status,
                );
                let unsafe_info = colorize(
                    table_row(&pack_metrics, &rs_files_used),
                    &detection_status,
                );
                let shift_chars = unsafe_info.chars().count() + 4;
                print!("{}  {: <2}", unsafe_info, icon);

                // Here comes some special control characters to position the cursor
                // properly for printing the last column containing the tree vines, after
                // the emoji icon. This is a workaround for a potential bug where the
                // radiation emoji will visually cover two characters in width but only
                // count as a single character if using the column formatting provided by
                // Rust. This could be unrelated to Rust and a quirk of this particular
                // symbol or something in the Terminal app on macOS.
                if emoji_symbols.will_output_emoji() {
                    print!("\r"); // Return the cursor to the start of the line.
                    print!("\x1B[{}C", shift_chars); // Move the cursor to the right so that it points to the icon character.
                }

                println!(" {}{}", tree_vines, pack_name);
            }
            TextTreeLine::ExtraDepsGroup { kind, tree_vines } => {
                let name = get_kind_group_name(kind);
                if name.is_none() {
                    continue;
                }
                let name = name.unwrap();

                // TODO: Fix the alignment on macOS (others too?)
                println!("{}{}{}", table_row_empty(), tree_vines, name);
            }
        }
    }

    println!();
    let total_detection_status = match (
        total_packs_none_detected_forbids_unsafe > 0,
        total_packs_none_detected_allows_unsafe > 0,
        total_packs_unsafe_detected > 0,
    ) {
        (_, _, true) => DetectionStatus::UnsafeDetected,
        (true, false, false) => DetectionStatus::NoneDetectedForbidsUnsafe,
        _ => DetectionStatus::NoneDetectedAllowsUnsafe,
    };
    println!(
        "{}",
        table_footer(total, total_unused, total_detection_status)
    );

    warning_count
}

// ---------- END: Public items ----------

fn table_footer(
    used: CounterBlock,
    not_used: CounterBlock,
    status: DetectionStatus,
) -> colored::ColoredString {
    let fmt = |used: &Count, not_used: &Count| {
        format!("{}/{}", used.unsafe_, used.unsafe_ + not_used.unsafe_)
    };
    let output = format!(
        "{: <10} {: <12} {: <6} {: <7} {: <7}",
        fmt(&used.functions, &not_used.functions),
        fmt(&used.exprs, &not_used.exprs),
        fmt(&used.item_impls, &not_used.item_impls),
        fmt(&used.item_traits, &not_used.item_traits),
        fmt(&used.methods, &not_used.methods),
    );
    colorize(output, &status)
}

fn table_row(pms: &PackageMetrics, rs_files_used: &HashSet<PathBuf>) -> String {
    let mut used = CounterBlock::default();
    let mut not_used = CounterBlock::default();
    for (k, v) in pms.rs_path_to_metrics.iter() {
        let target = if rs_files_used.contains(k) {
            &mut used
        } else {
            &mut not_used
        };
        *target = target.clone() + v.metrics.counters.clone();
    }
    let fmt = |used: &Count, not_used: &Count| {
        format!("{}/{}", used.unsafe_, used.unsafe_ + not_used.unsafe_)
    };
    format!(
        "{: <10} {: <12} {: <6} {: <7} {: <7}",
        fmt(&used.functions, &not_used.functions),
        fmt(&used.exprs, &not_used.exprs),
        fmt(&used.item_impls, &not_used.item_impls),
        fmt(&used.item_traits, &not_used.item_traits),
        fmt(&used.methods, &not_used.methods),
    )
}

fn table_row_empty() -> String {
    " ".repeat(
        UNSAFE_COUNTERS_HEADER
            .iter()
            .take(5)
            .map(|s| s.len())
            .sum::<usize>()
            + UNSAFE_COUNTERS_HEADER.len()
            + 1,
    )
}
