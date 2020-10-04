use crate::format::print::{Prefix, PrintConfig};
use crate::graph::{Graph, Node};
use crate::tree::{get_tree_symbols, TextTreeLine};

use super::construct_tree_vines_string;

use cargo::core::dependency::DepKind;
use cargo::core::PackageId;
use petgraph::visit::EdgeRef;
use petgraph::EdgeDirection;
use std::collections::{HashMap, HashSet};

/// Printing the returned TextTreeLines in order is expected to produce a nice
/// looking tree structure.
///
/// TODO: Return a impl Iterator<Item = TextTreeLine ... >
/// TODO: Consider separating the tree vine building from the tree traversal.
///
pub fn walk_dependency_tree(
    root_pack_id: PackageId,
    graph: &Graph,
    print_config: &PrintConfig,
) -> Vec<TextTreeLine> {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];
    let node = &graph.graph[graph.nodes[&root_pack_id]];
    walk_dependency_node(
        node,
        graph,
        &mut visited_deps,
        &mut levels_continue,
        print_config,
    )
}

fn construct_dependency_type_nodes_hashmap<'a>(
    graph: &'a Graph,
    package: &Node,
    print_config: &PrintConfig,
) -> HashMap<DepKind, Vec<&'a Node>> {
    let mut dependency_type_nodes: HashMap<DepKind, Vec<&Node>> = [
        (DepKind::Build, vec![]),
        (DepKind::Development, vec![]),
        (DepKind::Normal, vec![]),
    ]
    .iter()
    .cloned()
    .collect();

    for edge in graph
        .graph
        .edges_directed(graph.nodes[&package.id], print_config.direction)
    {
        let dependency = match print_config.direction {
            EdgeDirection::Incoming => &graph.graph[edge.source()],
            EdgeDirection::Outgoing => &graph.graph[edge.target()],
        };

        dependency_type_nodes
            .get_mut(edge.weight())
            .unwrap()
            .push(dependency);
    }

    dependency_type_nodes
}

fn walk_dependency_kind(
    kind: DepKind,
    deps: &mut Vec<&Node>,
    graph: &Graph,
    visited_deps: &mut HashSet<PackageId>,
    levels_continue: &mut Vec<bool>,
    print_config: &PrintConfig,
) -> Vec<TextTreeLine> {
    if deps.is_empty() {
        return Vec::new();
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    deps.sort_by_key(|n| n.id);

    let tree_symbols = get_tree_symbols(print_config.charset);
    let mut output = Vec::new();
    if let Prefix::Indent = print_config.prefix {
        match kind {
            DepKind::Normal => (),
            _ => {
                let mut tree_vines = String::new();
                for &continues in &**levels_continue {
                    let c = if continues { tree_symbols.down } else { " " };
                    tree_vines.push_str(&format!("{}   ", c));
                }
                output.push(TextTreeLine::ExtraDepsGroup { kind, tree_vines });
            }
        }
    }

    let mut node_iterator = deps.iter().peekable();
    while let Some(dependency) = node_iterator.next() {
        levels_continue.push(node_iterator.peek().is_some());
        output.append(&mut walk_dependency_node(
            dependency,
            graph,
            visited_deps,
            levels_continue,
            print_config,
        ));
        levels_continue.pop();
    }
    output
}

fn walk_dependency_node(
    package: &Node,
    graph: &Graph,
    visited_deps: &mut HashSet<PackageId>,
    levels_continue: &mut Vec<bool>,
    print_config: &PrintConfig,
) -> Vec<TextTreeLine> {
    let new = print_config.all || visited_deps.insert(package.id);
    let tree_vines = construct_tree_vines_string(levels_continue, print_config);

    let mut all_out_text_tree_lines = vec![TextTreeLine::Package {
        id: package.id,
        tree_vines,
    }];

    if !new {
        return all_out_text_tree_lines;
    }

    let mut dependency_type_nodes =
        construct_dependency_type_nodes_hashmap(graph, package, print_config);

    for (dep_kind, nodes) in dependency_type_nodes.iter_mut() {
        let mut dep_kind_out = walk_dependency_kind(
            *dep_kind,
            nodes,
            graph,
            visited_deps,
            levels_continue,
            print_config,
        );

        all_out_text_tree_lines.append(&mut dep_kind_out);
    }

    all_out_text_tree_lines
}
