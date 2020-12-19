use crate::format::emoji_symbols::EmojiSymbols;
use crate::format::print_config::{colorize, OutputFormat};
use crate::format::{get_kind_group_name, CrateDetectionStatus, SymbolKind};
use crate::mapping::CargoMetadataParameters;
use crate::scan::unsafe_stats;

use super::total_package_counts::TotalPackageCounts;
use super::TableParameters;
use super::{table_row, table_row_empty};

use cargo_metadata::{DependencyKind, PackageId};
use std::collections::HashSet;

pub struct HandlePackageParameters<'a> {
    pub total_package_counts: &'a mut TotalPackageCounts,
    pub visited_package_ids: &'a mut HashSet<PackageId>,
    pub warning_count: &'a mut u64,
}

pub fn handle_text_tree_line_extra_deps_group(
    dep_kind: DependencyKind,
    table_lines: &mut Vec<String>,
    tree_vines: String,
) {
    let name = get_kind_group_name(dep_kind);
    if name.is_none() {
        return;
    }
    let name = name.unwrap();

    // TODO: Fix the alignment on macOS (others too?)
    table_lines.push(format!("{}{}{}", table_row_empty(), tree_vines, name));
}

pub fn handle_text_tree_line_package(
    cargo_metadata_parameters: &CargoMetadataParameters,
    emoji_symbols: &EmojiSymbols,
    handle_package_parameters: &mut HandlePackageParameters,
    package_id: PackageId,
    table_lines: &mut Vec<String>,
    table_parameters: &TableParameters,
    tree_vines: String,
) {
    let package_is_new = handle_package_parameters
        .visited_package_ids
        .insert(package_id.clone());

    let package_metrics = match table_parameters
        .geiger_context
        .package_id_to_metrics
        .get(&package_id)
    {
        Some(m) => m,
        None => {
            *handle_package_parameters.warning_count += package_is_new as u64;
            eprintln!("WARNING: No metrics found for package: {}", package_id);
            return;
        }
    };
    let unsafe_info =
        unsafe_stats(package_metrics, table_parameters.rs_files_used);
    if package_is_new {
        handle_package_parameters
            .total_package_counts
            .total_counter_block += unsafe_info.used.clone();
        handle_package_parameters
            .total_package_counts
            .total_unused_counter_block += unsafe_info.unused.clone();
    }
    let unsafe_found = unsafe_info.used.has_unsafe();
    let crate_forbids_unsafe = unsafe_info.forbids_unsafe;
    let total_inc = package_is_new as i32;
    let crate_detection_status =
        get_crate_detection_status_and_update_package_counts(
            crate_forbids_unsafe,
            handle_package_parameters,
            total_inc,
            unsafe_found,
        );

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
        &crate_detection_status,
        table_parameters.print_config.output_format,
        format!(
            "{}",
            table_parameters
                .print_config
                .format
                .display(cargo_metadata_parameters, &package_id)
        ),
    );
    let unsafe_info = colorize(
        &crate_detection_status,
        table_parameters.print_config.output_format,
        table_row(&unsafe_info.used, &unsafe_info.unused),
    );

    let shift_chars = unsafe_info.chars().count() + 4;

    let mut line = String::new();
    line.push_str(format!("{}  {: <2}", unsafe_info, icon).as_str());

    // Here comes some special control characters to position the cursor
    // properly for printing the last column containing the tree vines, after
    // the emoji icon. This is a workaround for a potential bug where the
    // radiation emoji will visually cover two characters in width but only
    // count as a single character if using the column formatting provided by
    // Rust. This could be unrelated to Rust and a quirk of this particular
    // symbol or something in the Terminal app on macOS.
    if emoji_symbols.will_output_emoji()
        && table_parameters.print_config.output_format
            != OutputFormat::GitHubMarkdown
    {
        line.push('\r'); // Return the cursor to the start of the line.
        line.push_str(format!("\x1B[{}C", shift_chars).as_str()); // Move the cursor to the right so that it points to the icon character.
    } else if table_parameters.print_config.output_format
        == OutputFormat::GitHubMarkdown
        && crate_detection_status == CrateDetectionStatus::UnsafeDetected
    {
        // When rendering output in the GitHubMarkdown format, the Rads symbol
        // is only rendered as a single char, needing an extra space
        line.push_str(" ");
    }

    table_lines.push(format!("{} {}{}", line, tree_vines, package_name));
}

fn get_crate_detection_status_and_update_package_counts(
    crate_forbids_unsafe: bool,
    handle_package_parameters: &mut HandlePackageParameters,
    total_inc: i32,
    unsafe_found: bool,
) -> CrateDetectionStatus {
    match (crate_forbids_unsafe, unsafe_found) {
        (true, false) => {
            handle_package_parameters
                .total_package_counts
                .none_detected_forbids_unsafe += total_inc;
            CrateDetectionStatus::NoneDetectedForbidsUnsafe
        }
        (false, false) => {
            handle_package_parameters
                .total_package_counts
                .none_detected_allows_unsafe += total_inc;
            CrateDetectionStatus::NoneDetectedAllowsUnsafe
        }
        (_, true) => {
            handle_package_parameters
                .total_package_counts
                .unsafe_detected += total_inc;
            CrateDetectionStatus::UnsafeDetected
        }
    }
}

#[cfg(test)]
mod handle_text_tree_line_tests {
    use super::*;

    use rstest::*;

    #[rstest(
        input_dep_kind,
        expected_kind_group_name,
        case(
            DependencyKind::Build,
            Some(String::from("[build-dependencies]"))
        ),
        case(
            DependencyKind::Development,
            Some(String::from("[dev-dependencies]"))
        ),
        case(DependencyKind::Normal, None)
    )]
    fn handle_text_tree_line_extra_deps_group_test(
        input_dep_kind: DependencyKind,
        expected_kind_group_name: Option<String>,
    ) {
        let mut table_lines = Vec::<String>::new();

        let tree_vines = String::from("tree_vines");

        handle_text_tree_line_extra_deps_group(
            input_dep_kind,
            &mut table_lines,
            tree_vines.clone(),
        );

        if expected_kind_group_name.is_some() {
            assert_eq!(table_lines.len(), 1);
            assert_eq!(
                table_lines.first().unwrap().as_str(),
                format!(
                    "{}{}{}",
                    table_row_empty(),
                    tree_vines,
                    expected_kind_group_name.unwrap(),
                )
            );
        } else {
            assert!(table_lines.is_empty());
        }
    }

    #[rstest(
        input_crate_forbids_unsafe,
        input_total_inc,
        input_unsafe_found,
        expected_crate_detection_status,
        expected_none_detected_forbids_unsafe,
        expected_none_detected_allows_unsafe,
        expected_unsafe_detected,
        case(
            true,
            1,
            false,
            CrateDetectionStatus::NoneDetectedForbidsUnsafe,
            1,
            0,
            0
        ),
        case(
            true,
            0,
            false,
            CrateDetectionStatus::NoneDetectedForbidsUnsafe,
            0,
            0,
            0
        ),
        case(
            false,
            1,
            false,
            CrateDetectionStatus::NoneDetectedAllowsUnsafe,
            0,
            1,
            0
        ),
        case(
            false,
            0,
            false,
            CrateDetectionStatus::NoneDetectedAllowsUnsafe,
            0,
            0,
            0
        ),
        case(false, 1, true, CrateDetectionStatus::UnsafeDetected, 0, 0, 1),
        case(false, 0, true, CrateDetectionStatus::UnsafeDetected, 0, 0, 0)
    )]
    fn get_crate_detection_status_and_update_package_counts_test(
        input_crate_forbids_unsafe: bool,
        input_total_inc: i32,
        input_unsafe_found: bool,
        expected_crate_detection_status: CrateDetectionStatus,
        expected_none_detected_forbids_unsafe: i32,
        expected_none_detected_allows_unsafe: i32,
        expected_unsafe_detected: i32,
    ) {
        let mut handle_package_parameters = HandlePackageParameters {
            total_package_counts: &mut TotalPackageCounts {
                none_detected_forbids_unsafe: 0,
                none_detected_allows_unsafe: 0,
                unsafe_detected: 0,
                total_counter_block: Default::default(),
                total_unused_counter_block: Default::default(),
            },
            visited_package_ids: &mut Default::default(),
            warning_count: &mut 0,
        };

        let crate_detection_status =
            get_crate_detection_status_and_update_package_counts(
                input_crate_forbids_unsafe,
                &mut handle_package_parameters,
                input_total_inc,
                input_unsafe_found,
            );

        assert_eq!(crate_detection_status, expected_crate_detection_status);

        assert_eq!(
            handle_package_parameters
                .total_package_counts
                .none_detected_forbids_unsafe,
            expected_none_detected_forbids_unsafe
        );

        assert_eq!(
            handle_package_parameters
                .total_package_counts
                .none_detected_allows_unsafe,
            expected_none_detected_allows_unsafe
        );

        assert_eq!(
            handle_package_parameters
                .total_package_counts
                .unsafe_detected,
            expected_unsafe_detected
        );
    }
}
