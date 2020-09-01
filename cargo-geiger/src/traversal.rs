use crate::format::print::{Prefix, PrintConfig};
use crate::format::tree::{get_tree_symbols, TextTreeLine};
use crate::graph::{Graph, Node};

use cargo::core::dependency::DepKind;
use cargo::core::PackageId;
use petgraph::visit::EdgeRef;
use petgraph::EdgeDirection;
use std::collections::HashSet;

// ---------- BEGIN: Public items ----------

/// To print the returned TextTreeLines in order are expected to produce a nice
/// looking tree structure.
///
/// TODO: Return a impl Iterator<Item = TextTreeLine ... >
/// TODO: Consider separating the tree vine building from the tree traversal.
///
pub fn walk_dependency_tree(
    root_pack_id: PackageId,
    graph: &Graph,
    pc: &PrintConfig,
) -> Vec<TextTreeLine> {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];
    let node = &graph.graph[graph.nodes[&root_pack_id]];
    walk_dependency_node(
        node,
        graph,
        &mut visited_deps,
        &mut levels_continue,
        pc,
    )
}

// ---------- END: Public items ----------

fn walk_dependency_kind(
    kind: DepKind,
    deps: &mut Vec<&Node>,
    graph: &Graph,
    visited_deps: &mut HashSet<PackageId>,
    levels_continue: &mut Vec<bool>,
    pc: &PrintConfig,
) -> Vec<TextTreeLine> {
    if deps.is_empty() {
        return Vec::new();
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    deps.sort_by_key(|n| n.id);

    let tree_symbols = get_tree_symbols(pc.charset);
    let mut output = Vec::new();
    if let Prefix::Indent = pc.prefix {
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

    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        output.append(&mut walk_dependency_node(
            dependency,
            graph,
            visited_deps,
            levels_continue,
            pc,
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
    pc: &PrintConfig,
) -> Vec<TextTreeLine> {
    let new = pc.all || visited_deps.insert(package.id);
    let tree_symbols = get_tree_symbols(pc.charset);
    let tree_vines = match pc.prefix {
        Prefix::Depth => format!("{} ", levels_continue.len()),
        Prefix::Indent => {
            let mut buf = String::new();
            if let Some((&last_continues, rest)) = levels_continue.split_last()
            {
                for &continues in rest {
                    let c = if continues { tree_symbols.down } else { " " };
                    buf.push_str(&format!("{}   ", c));
                }
                let c = if last_continues {
                    tree_symbols.tee
                } else {
                    tree_symbols.ell
                };
                buf.push_str(&format!("{0}{1}{1} ", c, tree_symbols.right));
            }
            buf
        }
        Prefix::None => "".into(),
    };

    let mut all_out = vec![TextTreeLine::Package {
        id: package.id,
        tree_vines,
    }];

    if !new {
        return all_out;
    }

    let mut normal = vec![];
    let mut build = vec![];
    let mut development = vec![];
    for edge in graph
        .graph
        .edges_directed(graph.nodes[&package.id], pc.direction)
    {
        let dep = match pc.direction {
            EdgeDirection::Incoming => &graph.graph[edge.source()],
            EdgeDirection::Outgoing => &graph.graph[edge.target()],
        };
        match *edge.weight() {
            DepKind::Normal => normal.push(dep),
            DepKind::Build => build.push(dep),
            DepKind::Development => development.push(dep),
        }
    }
    let mut normal_out = walk_dependency_kind(
        DepKind::Normal,
        &mut normal,
        graph,
        visited_deps,
        levels_continue,
        pc,
    );
    let mut build_out = walk_dependency_kind(
        DepKind::Build,
        &mut build,
        graph,
        visited_deps,
        levels_continue,
        pc,
    );
    let mut dev_out = walk_dependency_kind(
        DepKind::Development,
        &mut development,
        graph,
        visited_deps,
        levels_continue,
        pc,
    );
    all_out.append(&mut normal_out);
    all_out.append(&mut build_out);
    all_out.append(&mut dev_out);
    all_out
}
