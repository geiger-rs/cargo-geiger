use crate::find::GeigerContext;
use crate::format::print::{colorize, PrintConfig};
use crate::format::tree::TextTreeLine;
use crate::format::{
    get_kind_group_name, CrateDetectionStatus, EmojiSymbols, SymbolKind,
};
use crate::rs_file::PackageMetrics;

use cargo::core::package::PackageSet;
use geiger::{Count, CounterBlock};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

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

pub fn create_table_from_text_tree_lines(
    geiger_context: &GeigerContext,
    package_set: &PackageSet,
    print_config: &PrintConfig,
    rs_files_used: &HashSet<PathBuf>,
    text_tree_lines: Vec<TextTreeLine>,
) -> (Vec<String>, u64) {
    let mut table_lines = Vec::<String>::new();

    let mut total_package_counts = TotalPackageCounts::new();

    let mut package_status = HashMap::new();
    let mut warning_count = 0;

    for text_tree_line in text_tree_lines {
        match text_tree_line {
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

                    for (path_buf, rs_file_metrics_wrapper) in
                        &pack_metrics.rs_path_to_metrics
                    {
                        let target = if rs_files_used.contains(path_buf) {
                            &mut total_package_counts.total_counter_block
                        } else {
                            &mut total_package_counts.total_unused_counter_block
                        };
                        *target = target.clone()
                            + rs_file_metrics_wrapper.metrics.counters.clone();
                    }

                    match (unsafe_found, crate_forbids_unsafe) {
                        (false, true) => {
                            total_package_counts
                                .none_detected_forbids_unsafe += 1;
                            CrateDetectionStatus::NoneDetectedForbidsUnsafe
                        }
                        (false, false) => {
                            total_package_counts.none_detected_allows_unsafe +=
                                1;
                            CrateDetectionStatus::NoneDetectedAllowsUnsafe
                        }
                        (true, _) => {
                            total_package_counts.unsafe_detected += 1;
                            CrateDetectionStatus::UnsafeDetected
                        }
                    }
                });

                let emoji_symbols = EmojiSymbols::new(print_config.charset);

                let crate_detection_status =
                    package_status.get(&id).unwrap_or_else(|| {
                        panic!("Expected to find package by id: {}", &id)
                    });
                let icon = match crate_detection_status {
                    CrateDetectionStatus::NoneDetectedForbidsUnsafe => {
                        emoji_symbols.emoji(SymbolKind::Lock)
                    }
                    CrateDetectionStatus::NoneDetectedAllowsUnsafe => {
                        emoji_symbols.emoji(SymbolKind::QuestionMark)
                    }
                    CrateDetectionStatus::UnsafeDetected => {
                        emoji_symbols.emoji(SymbolKind::Rads)
                    }
                };

                let package_name = colorize(
                    format!(
                        "{}",
                        print_config
                            .format
                            .display(&id, pack.manifest().metadata())
                    ),
                    &crate_detection_status,
                );
                let unsafe_info = colorize(
                    table_row(&pack_metrics, &rs_files_used),
                    &crate_detection_status,
                );

                let shift_chars = unsafe_info.chars().count() + 4;

                let mut line = String::new();
                line.push_str(
                    format!("{}  {: <2}", unsafe_info, icon).as_str(),
                );

                // Here comes some special control characters to position the cursor
                // properly for printing the last column containing the tree vines, after
                // the emoji icon. This is a workaround for a potential bug where the
                // radiation emoji will visually cover two characters in width but only
                // count as a single character if using the column formatting provided by
                // Rust. This could be unrelated to Rust and a quirk of this particular
                // symbol or something in the Terminal app on macOS.
                if emoji_symbols.will_output_emoji() {
                    line.push_str("\r"); // Return the cursor to the start of the line.
                    line.push_str(format!("\x1B[{}C", shift_chars).as_str()); // Move the cursor to the right so that it points to the icon character.
                }

                table_lines
                    .push(format!("{} {}{}", line, tree_vines, package_name))
            }
            TextTreeLine::ExtraDepsGroup { kind, tree_vines } => {
                let name = get_kind_group_name(kind);
                if name.is_none() {
                    continue;
                }
                let name = name.unwrap();

                // TODO: Fix the alignment on macOS (others too?)
                table_lines.push(format!(
                    "{}{}{}",
                    table_row_empty(),
                    tree_vines,
                    name
                ))
            }
        }
    }

    table_lines.push(String::new());
    let total_detection_status =
        total_package_counts.get_total_detection_status();

    table_lines.push(format!(
        "{}",
        table_footer(
            total_package_counts.total_counter_block,
            total_package_counts.total_unused_counter_block,
            total_detection_status
        )
    ));

    table_lines.push(String::new());

    (table_lines, warning_count)
}

struct TotalPackageCounts {
    none_detected_forbids_unsafe: i32,
    none_detected_allows_unsafe: i32,
    unsafe_detected: i32,
    total_counter_block: CounterBlock,
    total_unused_counter_block: CounterBlock,
}

impl TotalPackageCounts {
    fn new() -> TotalPackageCounts {
        TotalPackageCounts {
            none_detected_forbids_unsafe: 0,
            none_detected_allows_unsafe: 0,
            unsafe_detected: 0,
            total_counter_block: CounterBlock::default(),
            total_unused_counter_block: CounterBlock::default(),
        }
    }

    fn get_total_detection_status(&self) -> CrateDetectionStatus {
        match (
            self.none_detected_forbids_unsafe > 0,
            self.none_detected_allows_unsafe > 0,
            self.unsafe_detected > 0,
        ) {
            (_, _, true) => CrateDetectionStatus::UnsafeDetected,
            (true, false, false) => {
                CrateDetectionStatus::NoneDetectedForbidsUnsafe
            }
            _ => CrateDetectionStatus::NoneDetectedAllowsUnsafe,
        }
    }
}

fn table_footer(
    used: CounterBlock,
    not_used: CounterBlock,
    status: CrateDetectionStatus,
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
    for (path_buf, rs_file_metrics_wrapper) in pms.rs_path_to_metrics.iter() {
        let target = if rs_files_used.contains(path_buf) {
            &mut used
        } else {
            &mut not_used
        };
        *target =
            target.clone() + rs_file_metrics_wrapper.metrics.counters.clone();
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

#[cfg(test)]
mod table_tests {
    use super::*;

    use crate::rs_file::RsFileMetricsWrapper;

    use geiger::RsFileMetrics;
    use std::collections::HashMap;
    use std::path::Path;
    use strum::IntoEnumIterator;

    #[test]
    fn table_footer_test() {
        let used_counter_block = create_counter_block();
        let not_used_counter_block = create_counter_block();

        let expected_line =
            String::from("2/4        4/8          6/12   8/16    10/20  ");

        for crate_detection_status in CrateDetectionStatus::iter() {
            let table_footer = table_footer(
                used_counter_block.clone(),
                not_used_counter_block.clone(),
                crate_detection_status.clone(),
            );

            assert_eq!(
                table_footer,
                colorize(expected_line.clone(), &crate_detection_status)
            );
        }
    }

    #[test]
    fn table_row_test() {
        let mut rs_path_to_metrics =
            HashMap::<PathBuf, RsFileMetricsWrapper>::new();

        rs_path_to_metrics.insert(
            Path::new("package_1_path").to_path_buf(),
            create_rs_file_metrics_wrapper(true, true),
        );

        rs_path_to_metrics.insert(
            Path::new("package_2_path").to_path_buf(),
            create_rs_file_metrics_wrapper(true, false),
        );

        rs_path_to_metrics.insert(
            Path::new("package_3_path").to_path_buf(),
            create_rs_file_metrics_wrapper(false, false),
        );

        let package_metrics = PackageMetrics { rs_path_to_metrics };

        let rs_files_used: HashSet<PathBuf> = [
            Path::new("package_1_path").to_path_buf(),
            Path::new("package_3_path").to_path_buf(),
        ]
        .iter()
        .cloned()
        .collect();

        let table_row = table_row(&package_metrics, &rs_files_used);
        assert_eq!(table_row, "4/6        8/12         12/18  16/24   20/30  ");
    }

    #[test]
    fn table_row_empty_test() {
        let empty_table_row = table_row_empty();
        assert_eq!(empty_table_row.len(), 50);
    }

    #[test]
    fn total_package_counts_get_total_detection_status_tests() {
        let total_package_counts_unsafe_detected = TotalPackageCounts {
            none_detected_forbids_unsafe: 0,
            none_detected_allows_unsafe: 0,
            unsafe_detected: 1,
            total_counter_block: CounterBlock::default(),
            total_unused_counter_block: CounterBlock::default(),
        };

        assert_eq!(
            total_package_counts_unsafe_detected.get_total_detection_status(),
            CrateDetectionStatus::UnsafeDetected
        );

        let total_package_counts_none_detected_forbids_unsafe =
            TotalPackageCounts {
                none_detected_forbids_unsafe: 1,
                none_detected_allows_unsafe: 0,
                unsafe_detected: 0,
                total_counter_block: CounterBlock::default(),
                total_unused_counter_block: CounterBlock::default(),
            };

        assert_eq!(
            total_package_counts_none_detected_forbids_unsafe
                .get_total_detection_status(),
            CrateDetectionStatus::NoneDetectedForbidsUnsafe
        );

        let total_package_counts_none_detected_allows_unsafe =
            TotalPackageCounts {
                none_detected_forbids_unsafe: 4,
                none_detected_allows_unsafe: 1,
                unsafe_detected: 0,
                total_counter_block: CounterBlock::default(),
                total_unused_counter_block: CounterBlock::default(),
            };

        assert_eq!(
            total_package_counts_none_detected_allows_unsafe
                .get_total_detection_status(),
            CrateDetectionStatus::NoneDetectedAllowsUnsafe
        );
    }

    fn create_rs_file_metrics_wrapper(
        forbids_unsafe: bool,
        is_crate_entry_point: bool,
    ) -> RsFileMetricsWrapper {
        RsFileMetricsWrapper {
            metrics: RsFileMetrics {
                counters: create_counter_block(),
                forbids_unsafe,
            },
            is_crate_entry_point,
        }
    }

    fn create_counter_block() -> CounterBlock {
        CounterBlock {
            functions: Count {
                safe: 1,
                unsafe_: 2,
            },
            exprs: Count {
                safe: 3,
                unsafe_: 4,
            },
            item_impls: Count {
                safe: 5,
                unsafe_: 6,
            },
            item_traits: Count {
                safe: 7,
                unsafe_: 8,
            },
            methods: Count {
                safe: 9,
                unsafe_: 10,
            },
        }
    }
}
