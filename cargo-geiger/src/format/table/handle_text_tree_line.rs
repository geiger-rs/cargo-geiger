use crate::format::emoji_symbols::EmojiSymbols;
use crate::format::print_config::{colorize, OutputFormat};
use crate::format::{get_kind_group_name, CrateDetectionStatus, SymbolKind};
use crate::mapping::CargoMetadataParameters;
use crate::scan::unsafe_stats;

use super::total_package_counts::TotalPackageCounts;
use super::TableParameters;
use super::{table_row, table_row_empty};

use krates::cm::{DependencyKind, PackageId};
use colored::ColoredString;
use std::collections::HashSet;
use std::fmt::Display;

pub struct HandlePackageParameters<'a> {
    pub total_package_counts: &'a mut TotalPackageCounts,
    pub visited_package_ids: &'a mut HashSet<PackageId>,
    pub warning_count: &'a mut u64,
}

pub fn text_tree_line_extra_deps_group_to_table_line_string(
    dep_kind: DependencyKind,
    tree_vines: String,
) -> Option<String> {
    get_kind_group_name(dep_kind)
        .map(|name| format!("{}{}{}", table_row_empty(), tree_vines, name,))
}

pub fn text_tree_line_package_to_table_line_string(
    cargo_metadata_parameters: &CargoMetadataParameters,
    emoji_symbols: &EmojiSymbols,
    handle_package_parameters: &mut HandlePackageParameters,
    package_id: PackageId,
    table_parameters: &TableParameters,
    tree_vines: String,
) -> Option<String> {
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
            return None;
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
        table_row(
            &unsafe_info.used,
            &unsafe_info.unused,
            table_parameters.print_config.output_format,
        ),
    );

    Some(construct_package_text_tree_line(
        crate_detection_status,
        emoji_symbols,
        icon,
        package_name,
        table_parameters,
        tree_vines,
        unsafe_info,
    ))
}

fn construct_package_text_tree_line(
    crate_detection_status: CrateDetectionStatus,
    emoji_symbols: &EmojiSymbols,
    icon: Box<dyn Display>,
    package_name: ColoredString,
    table_parameters: &TableParameters,
    tree_vines: String,
    unsafe_info: ColoredString,
) -> String {
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
    match (
        emoji_symbols.will_output_emoji(),
        table_parameters.print_config.output_format,
        crate_detection_status,
    ) {
        (true, output_format, _)
            if output_format != OutputFormat::GitHubMarkdown =>
        {
            line.push('\r'); // Return the cursor to the start of the line.
            line.push_str(format!("\x1B[{}C", shift_chars).as_str()); // Move the cursor to the right so that it points to the icon character.
        }
        (
            _,
            OutputFormat::GitHubMarkdown,
            CrateDetectionStatus::UnsafeDetected,
        ) => {
            // When rendering output in the GitHubMarkdown format, the Rads symbol
            // is only rendered as a single char, needing an extra space
            line.push(' ');
        }
        _ => (),
    }

    format!("{} {}{}", line, tree_vines, package_name)
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

    use crate::format::print_config::PrintConfig;
    use colored::Colorize;
    use rstest::*;

    #[rstest(
        input_dep_kind,
        expected_table_line_option,
        case(
            DependencyKind::Build,
            Some(format!("{}{}{}", table_row_empty(), "tree_vines", "[build-dependencies]"))
        ),
        case(
            DependencyKind::Development,
            Some(format!("{}{}{}", table_row_empty(), "tree_vines", "[dev-dependencies]"))
        ),
        case(DependencyKind::Normal, None)
    )]
    fn text_tree_line_extra_deps_group_to_table_line_string_test(
        input_dep_kind: DependencyKind,
        expected_table_line_option: Option<String>,
    ) {
        let tree_vines = String::from("tree_vines");
        let actual_table_lines =
            text_tree_line_extra_deps_group_to_table_line_string(
                input_dep_kind,
                tree_vines,
            );

        assert_eq!(actual_table_lines, expected_table_line_option);
    }

    #[rstest(
        input_crate_detection_status,
        input_output_format,
        input_symbol_kind,
        expected_package_text_tree_line,
        case(
            CrateDetectionStatus::NoneDetectedForbidsUnsafe,
            OutputFormat::GitHubMarkdown,
            SymbolKind::Lock,
            String::from("unsafe_info  🔒  tree_vinespackage_name")
        ),
        case(
            CrateDetectionStatus::UnsafeDetected,
            OutputFormat::GitHubMarkdown,
            SymbolKind::Rads,
            String::from("unsafe_info  ☢\u{fe0f}  tree_vinespackage_name")
        )
    )]
    fn construct_package_text_tree_line_test(
        input_crate_detection_status: CrateDetectionStatus,
        input_output_format: OutputFormat,
        input_symbol_kind: SymbolKind,
        expected_package_text_tree_line: String,
    ) {
        let emoji_symbols = EmojiSymbols::new(input_output_format);
        let icon = emoji_symbols.emoji(input_symbol_kind);
        let package_name = String::from("package_name").normal();
        let table_parameters = TableParameters {
            geiger_context: &Default::default(),
            print_config: &PrintConfig {
                output_format: input_output_format,
                ..Default::default()
            },
            rs_files_used: &Default::default(),
        };
        let tree_vines = String::from("tree_vines");
        let unsafe_info = ColoredString::from("unsafe_info").normal();

        let package_text_tree_line = construct_package_text_tree_line(
            input_crate_detection_status,
            &emoji_symbols,
            icon,
            package_name,
            &table_parameters,
            tree_vines,
            unsafe_info,
        );

        assert_eq!(package_text_tree_line, expected_package_text_tree_line);
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
