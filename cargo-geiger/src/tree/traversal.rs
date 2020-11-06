mod dependency_kind;
mod dependency_node;

use crate::format::print_config::PrintConfig;
use crate::graph::Graph;
use crate::tree::TextTreeLine;

use super::construct_tree_vines_string;
use dependency_kind::walk_dependency_kind;
use dependency_node::walk_dependency_node;

use crate::krates_utils::{CargoMetadataParameters, ToPackageId};
use cargo::core::PackageSet;
use std::collections::HashSet;

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
    let node = &graph.graph[graph.nodes[&root_package_id
        .to_package_id(cargo_metadata_parameters.krates, package_set)]];
    walk_dependency_node(
        node,
        graph,
        &mut visited_deps,
        &mut levels_continue,
        print_config,
    )
}
