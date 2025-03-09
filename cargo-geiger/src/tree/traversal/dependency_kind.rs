use crate::format::print_config::Prefix;
use crate::mapping::CargoMetadataParameters;
use crate::tree::traversal::WalkDependencyParameters;
use crate::tree::{get_tree_symbols, TextTreeLine, TreeSymbols};

use super::dependency_node::walk_dependency_node;

use krates::cm::{DependencyKind, PackageId};
use std::fmt::Write as _;
use std::iter::Peekable;
use std::slice::Iter;

pub fn walk_dependency_kind(
    cargo_metadata_parameters: &CargoMetadataParameters,
    dep_kind: DependencyKind,
    deps: &mut [PackageId],
    walk_dependency_parameters: &mut WalkDependencyParameters,
) -> Vec<TextTreeLine> {
    if deps.is_empty() {
        return Vec::new();
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    deps.sort_by_key(|n| n.clone());

    let tree_symbols =
        get_tree_symbols(walk_dependency_parameters.print_config.output_format);
    let mut text_tree_lines = Vec::new();
    if let Prefix::Indent = walk_dependency_parameters.print_config.prefix {
        push_extra_deps_group_text_tree_line_for_non_normal_dependencies(
            dep_kind,
            walk_dependency_parameters.levels_continue,
            &tree_symbols,
            &mut text_tree_lines,
        )
    }

    let mut node_iterator = deps.iter().peekable();
    while let Some(dependency) = node_iterator.next() {
        handle_walk_dependency_node(
            cargo_metadata_parameters,
            dependency,
            &mut node_iterator,
            &mut text_tree_lines,
            walk_dependency_parameters,
        );
    }
    text_tree_lines
}

fn handle_walk_dependency_node(
    cargo_metadata_parameters: &CargoMetadataParameters,
    dependency: &PackageId,
    node_iterator: &mut Peekable<Iter<PackageId>>,
    text_tree_lines: &mut Vec<TextTreeLine>,
    walk_dependency_parameters: &mut WalkDependencyParameters,
) {
    walk_dependency_parameters
        .levels_continue
        .push(node_iterator.peek().is_some());
    text_tree_lines.append(&mut walk_dependency_node(
        cargo_metadata_parameters,
        dependency,
        walk_dependency_parameters,
    ));
    walk_dependency_parameters.levels_continue.pop();
}

fn push_extra_deps_group_text_tree_line_for_non_normal_dependencies(
    dep_kind: DependencyKind,
    levels_continue: &[bool],
    tree_symbols: &TreeSymbols,
    text_tree_lines: &mut Vec<TextTreeLine>,
) {
    match dep_kind {
        DependencyKind::Normal => (),
        _ => {
            let mut tree_vines = String::new();
            for &continues in levels_continue {
                let c = if continues { tree_symbols.down } else { " " };
                (write!(tree_vines, "{}   ", c)).unwrap()
            }
            text_tree_lines.push(TextTreeLine::ExtraDepsGroup {
                kind: dep_kind,
                tree_vines,
            });
        }
    }
}

#[cfg(test)]
mod traversal_tests {
    use super::*;

    use crate::format::print_config::OutputFormat;
    use crate::tree::TextTreeLine::ExtraDepsGroup;

    use rstest::*;

    #[rstest(
        input_dep_kind,
        input_levels_continue,
        expected_text_tree_lines,
        case(
            DependencyKind::Build,
            vec![],
            vec![
                ExtraDepsGroup {
                    kind: DependencyKind::Build,
                    tree_vines: String::from("")
                }
            ]
        ),
        case(
            DependencyKind::Build,
            vec![false, true],
            vec![
                ExtraDepsGroup {
                    kind: DependencyKind::Build,
                    tree_vines: format!(
                    "    {}   ",
                    get_tree_symbols(OutputFormat::Utf8).down
                    )
                }
            ]
        ),
        case(
            DependencyKind::Development,
            vec![true],
            vec![
                ExtraDepsGroup {
                    kind: DependencyKind::Development,
                    tree_vines: format!(
                    "{}   ",
                    get_tree_symbols(OutputFormat::Utf8).down
                    )
                }
            ]
        ),
        case(
            DependencyKind::Development,
            vec![false],
            vec![
                ExtraDepsGroup {
                    kind: DependencyKind::Development,
                    tree_vines: String::from("    ")
                }
            ]
        ),
        case(
            DependencyKind::Normal,
            vec![],
            vec![]
        )
    )]
    fn push_extra_deps_group_text_tree_line_for_non_normal_dependencies_test(
        input_dep_kind: DependencyKind,
        input_levels_continue: Vec<bool>,
        expected_text_tree_lines: Vec<TextTreeLine>,
    ) {
        let mut text_tree_lines: Vec<TextTreeLine> = vec![];
        let tree_symbols = get_tree_symbols(OutputFormat::Utf8);

        push_extra_deps_group_text_tree_line_for_non_normal_dependencies(
            input_dep_kind,
            &input_levels_continue,
            &tree_symbols,
            &mut text_tree_lines,
        );

        assert_eq!(text_tree_lines, expected_text_tree_lines);
    }
}
