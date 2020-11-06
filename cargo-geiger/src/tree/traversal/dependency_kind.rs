use crate::format::print_config::{Prefix, PrintConfig};
use crate::graph::Graph;
use crate::tree::{get_tree_symbols, TextTreeLine, TreeSymbols};

use super::dependency_node::walk_dependency_node;

use cargo::core::dependency::DepKind;
use cargo::core::PackageId;
use std::collections::HashSet;
use std::iter::Peekable;
use std::slice::Iter;

pub fn walk_dependency_kind(
    dep_kind: DepKind,
    deps: &mut Vec<&PackageId>,
    graph: &Graph,
    visited_deps: &mut HashSet<PackageId>,
    levels_continue: &mut Vec<bool>,
    print_config: &PrintConfig,
) -> Vec<TextTreeLine> {
    if deps.is_empty() {
        return Vec::new();
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    deps.sort_by_key(|n| *n);

    let tree_symbols = get_tree_symbols(print_config.charset);
    let mut text_tree_lines = Vec::new();
    if let Prefix::Indent = print_config.prefix {
        push_extra_deps_group_text_tree_line_for_non_normal_dependencies(
            dep_kind,
            levels_continue,
            &tree_symbols,
            &mut text_tree_lines,
        )
    }

    let mut node_iterator = deps.iter().peekable();
    while let Some(dependency) = node_iterator.next() {
        handle_walk_dependency_node(
            dependency,
            graph,
            levels_continue,
            &mut node_iterator,
            print_config,
            &mut text_tree_lines,
            visited_deps,
        );
    }
    text_tree_lines
}

fn handle_walk_dependency_node(
    dependency: &PackageId,
    graph: &Graph,
    levels_continue: &mut Vec<bool>,
    node_iterator: &mut Peekable<Iter<&PackageId>>,
    print_config: &PrintConfig,
    text_tree_lines: &mut Vec<TextTreeLine>,
    visited_deps: &mut HashSet<PackageId>,
) {
    levels_continue.push(node_iterator.peek().is_some());
    text_tree_lines.append(&mut walk_dependency_node(
        dependency,
        graph,
        visited_deps,
        levels_continue,
        print_config,
    ));
    levels_continue.pop();
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
