mod handle_text_tree_line;
mod total_package_counts;

use crate::format::emoji_symbols::EmojiSymbols;
use crate::format::print_config::{colorize, PrintConfig};
use crate::format::CrateDetectionStatus;
use crate::mapping::CargoMetadataParameters;
use crate::scan::{GeigerContext, ScanResult};
use crate::tree::TextTreeLine;

use handle_text_tree_line::{
    handle_text_tree_line_extra_deps_group, handle_text_tree_line_package,
    HandlePackageParameters,
};
use total_package_counts::TotalPackageCounts;

use cargo_geiger_serde::{Count, CounterBlock};
use std::collections::HashSet;
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
    cargo_metadata_parameters: &CargoMetadataParameters,
    table_parameters: &TableParameters,
    text_tree_lines: Vec<TextTreeLine>,
) -> ScanResult {
    let mut table_lines = Vec::<String>::new();
    let mut total_package_counts = TotalPackageCounts::new();
    let mut warning_count = 0;
    let mut visited_package_ids = HashSet::new();
    let emoji_symbols =
        EmojiSymbols::new(table_parameters.print_config.charset);
    let mut handle_package_parameters = HandlePackageParameters {
        total_package_counts: &mut total_package_counts,
        visited_package_ids: &mut visited_package_ids,
        warning_count: &mut warning_count,
    };

    for text_tree_line in text_tree_lines {
        match text_tree_line {
            TextTreeLine::ExtraDepsGroup {
                kind: dep_kind,
                tree_vines,
            } => handle_text_tree_line_extra_deps_group(
                dep_kind,
                &mut table_lines,
                tree_vines,
            ),
            TextTreeLine::Package {
                id: package_id,
                tree_vines,
            } => handle_text_tree_line_package(
                cargo_metadata_parameters,
                &emoji_symbols,
                &mut handle_package_parameters,
                package_id,
                &mut table_lines,
                table_parameters,
                tree_vines,
            ),
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

    ScanResult {
        scan_output_lines: table_lines,
        warning_count,
    }
}

pub struct TableParameters<'a> {
    pub geiger_context: &'a GeigerContext,
    pub print_config: &'a PrintConfig,
    pub rs_files_used: &'a HashSet<PathBuf>,
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

fn table_row(used: &CounterBlock, not_used: &CounterBlock) -> String {
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
    let headers_but_last =
        &UNSAFE_COUNTERS_HEADER[..UNSAFE_COUNTERS_HEADER.len() - 1];
    let n = headers_but_last
        .iter()
        .map(|s| s.len())
        .sum::<usize>()
        + headers_but_last.len() // Space after each column
        + 2 // Unsafety symbol width
        + 1; // Space after symbol
    " ".repeat(n)
}

#[cfg(test)]
mod table_tests {
    use super::*;

    use crate::scan::{unsafe_stats, PackageMetrics, RsFileMetricsWrapper};

    use geiger::RsFileMetrics;
    use rstest::*;
    use std::collections::HashMap;
    use std::path::Path;
    use strum::IntoEnumIterator;

    #[rstest]
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

    #[rstest]
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
        let unsafety = unsafe_stats(&package_metrics, &rs_files_used);

        let table_row = table_row(&unsafety.used, &unsafety.unused);
        assert_eq!(table_row, "4/6        8/12         12/18  16/24   20/30  ");
    }

    #[rstest]
    fn table_row_empty_test() {
        let empty_table_row = table_row_empty();
        assert_eq!(empty_table_row.len(), 51);
    }

    #[rstest(
        input_none_detected_forbids_unsafe,
        input_none_detected_allows_unsafe,
        input_unsafe_detected,
        expected_crate_detection_status,
        case(0, 0, 1, CrateDetectionStatus::UnsafeDetected),
        case(1, 0, 0, CrateDetectionStatus::NoneDetectedForbidsUnsafe),
        case(4, 1, 0, CrateDetectionStatus::NoneDetectedAllowsUnsafe)
    )]
    fn total_package_counts_get_total_detection_status_tests(
        input_none_detected_forbids_unsafe: i32,
        input_none_detected_allows_unsafe: i32,
        input_unsafe_detected: i32,
        expected_crate_detection_status: CrateDetectionStatus,
    ) {
        let total_detection_status = TotalPackageCounts {
            none_detected_forbids_unsafe: input_none_detected_forbids_unsafe,
            none_detected_allows_unsafe: input_none_detected_allows_unsafe,
            unsafe_detected: input_unsafe_detected,
            total_counter_block: CounterBlock::default(),
            total_unused_counter_block: CounterBlock::default(),
        };

        assert_eq!(
            total_detection_status.get_total_detection_status(),
            expected_crate_detection_status
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
