use crate::format::print_config::Prefix;
use crate::tree::traversal::WalkDependencyParameters;
use crate::tree::{get_tree_symbols, TextTreeLine, TreeSymbols};

use super::dependency_node::walk_dependency_node;

use crate::mapping::CargoMetadataParameters;
use cargo::core::dependency::DepKind;
use cargo::core::PackageSet;
use std::iter::Peekable;
use std::slice::Iter;

pub fn walk_dependency_kind(
    cargo_metadata_parameters: &CargoMetadataParameters,
    dep_kind: DepKind,
    deps: &mut Vec<cargo_metadata::PackageId>,
    package_set: &PackageSet,
    walk_dependency_parameters: &mut WalkDependencyParameters,
) -> Vec<TextTreeLine> {
    if deps.is_empty() {
        return Vec::new();
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    deps.sort_by_key(|n| n.clone());

    let tree_symbols =
        get_tree_symbols(walk_dependency_parameters.print_config.charset);
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
            package_set,
            &mut text_tree_lines,
            walk_dependency_parameters,
        );
    }
    text_tree_lines
}

fn handle_walk_dependency_node(
    cargo_metadata_parameters: &CargoMetadataParameters,
    dependency: &cargo_metadata::PackageId,
    node_iterator: &mut Peekable<Iter<cargo_metadata::PackageId>>,
    package_set: &PackageSet,
    text_tree_lines: &mut Vec<TextTreeLine>,
    walk_dependency_parameters: &mut WalkDependencyParameters,
) {
    walk_dependency_parameters
        .levels_continue
        .push(node_iterator.peek().is_some());
    text_tree_lines.append(&mut walk_dependency_node(
        cargo_metadata_parameters,
        dependency,
        package_set,
        walk_dependency_parameters,
    ));
    walk_dependency_parameters.levels_continue.pop();
}

fn push_extra_deps_group_text_tree_line_for_non_normal_dependencies(
    dep_kind: DepKind,
    levels_continue: &[bool],
    tree_symbols: &TreeSymbols,
    text_tree_lines: &mut Vec<TextTreeLine>,
) {
    match dep_kind {
        DepKind::Normal => (),
        _ => {
            let mut tree_vines = String::new();
            for &continues in &*levels_continue {
                let c = if continues { tree_symbols.down } else { " " };
                tree_vines.push_str(&format!("{}   ", c))
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

    use crate::format::Charset;
    use crate::tree::TextTreeLine::ExtraDepsGroup;

    use rstest::*;

    #[rstest(
        input_dep_kind,
        input_levels_continue,
        expected_text_tree_lines,
        case(
            DepKind::Build,
            vec![],
            vec![
                ExtraDepsGroup {
                    kind: DepKind::Build,
                    tree_vines: String::from("")
                }
            ]
        ),
        case(
            DepKind::Build,
            vec![false, true],
            vec![
                ExtraDepsGroup {
                    kind: DepKind::Build,
                    tree_vines: format!(
                    "    {}   ",
                    get_tree_symbols(Charset::Utf8).down
                    )
                }
            ]
        ),
        case(
            DepKind::Development,
            vec![true],
            vec![
                ExtraDepsGroup {
                    kind: DepKind::Development,
                    tree_vines: format!(
                    "{}   ",
                    get_tree_symbols(Charset::Utf8).down
                    )
                }
            ]
        ),
        case(
            DepKind::Development,
            vec![false],
            vec![
                ExtraDepsGroup {
                    kind: DepKind::Development,
                    tree_vines: String::from("    ")
                }
            ]
        ),
        case(
            DepKind::Normal,
            vec![],
            vec![]
        )
    )]
    fn push_extra_deps_group_text_tree_line_for_non_normal_dependencies_test(
        input_dep_kind: DepKind,
        input_levels_continue: Vec<bool>,
        expected_text_tree_lines: Vec<TextTreeLine>,
    ) {
        let mut text_tree_lines: Vec<TextTreeLine> = vec![];
        let tree_symbols = get_tree_symbols(Charset::Utf8);

        push_extra_deps_group_text_tree_line_for_non_normal_dependencies(
            input_dep_kind,
            &input_levels_continue,
            &tree_symbols,
            &mut text_tree_lines,
        );

        assert_eq!(text_tree_lines, expected_text_tree_lines);
    }
}
