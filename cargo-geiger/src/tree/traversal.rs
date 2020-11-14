mod dependency_kind;
mod dependency_node;

use crate::format::print_config::PrintConfig;
use crate::graph::Graph;
use crate::tree::TextTreeLine;
use crate::utils::CargoMetadataParameters;

use super::construct_tree_vines_string;
use dependency_kind::walk_dependency_kind;
use dependency_node::walk_dependency_node;

use cargo::core::PackageSet;
use std::collections::HashSet;

pub struct WalkDependencyParameters<'a> {
    pub graph: &'a Graph,
    pub levels_continue: &'a mut Vec<bool>,
    pub print_config: &'a PrintConfig,
    pub visited_deps: &'a mut HashSet<cargo_metadata::PackageId>,
}

/// Printing the returned TextTreeLines in order is expected to produce a nice
/// looking tree structure.
///
/// TODO: Return a impl Iterator<Item = TextTreeLine ... >
/// TODO: Consider separating the tree vine building from the tree traversal.
///
pub fn walk_dependency_tree(
    cargo_metadata_parameters: &CargoMetadataParameters,
    graph: &Graph,
    package_set: &PackageSet,
    print_config: &PrintConfig,
    root_package_id: cargo_metadata::PackageId,
) -> Vec<TextTreeLine> {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];

    let mut walk_dependency_paramters = WalkDependencyParameters {
        graph,
        levels_continue: &mut levels_continue,
        print_config,
        visited_deps: &mut visited_deps,
    };

    let node = &graph.graph[graph.nodes[&root_package_id]];
    walk_dependency_node(
        cargo_metadata_parameters,
        &node,
        package_set,
        &mut walk_dependency_paramters,
    )
}
