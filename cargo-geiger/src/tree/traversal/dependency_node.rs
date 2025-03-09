use crate::format::print_config::PrintConfig;
use crate::graph::Graph;
use crate::mapping::CargoMetadataParameters;
use crate::tree::traversal::WalkDependencyParameters;
use crate::tree::TextTreeLine;

use super::construct_tree_vines_string;
use super::walk_dependency_kind;

use krates::cm::{DependencyKind, PackageId};
use petgraph::visit::EdgeRef;
use petgraph::EdgeDirection;
use std::collections::HashMap;

pub fn walk_dependency_node(
    cargo_metadata_parameters: &CargoMetadataParameters,
    package: &PackageId,
    walk_dependency_parameters: &mut WalkDependencyParameters,
) -> Vec<TextTreeLine> {
    let new = walk_dependency_parameters.print_config.all
        || walk_dependency_parameters
            .visited_deps
            .insert(package.clone());
    let tree_vines = construct_tree_vines_string(
        walk_dependency_parameters.levels_continue,
        walk_dependency_parameters.print_config,
    );

    let mut all_out_text_tree_lines = vec![TextTreeLine::Package {
        id: package.clone(),
        tree_vines,
    }];

    if !new {
        return all_out_text_tree_lines;
    }

    let mut dependency_type_nodes = construct_dependency_type_nodes_hashmap(
        walk_dependency_parameters.graph,
        package,
        walk_dependency_parameters.print_config,
    );

    for (dependency_kind, nodes) in dependency_type_nodes.iter_mut() {
        let mut dep_kind_out = walk_dependency_kind(
            cargo_metadata_parameters,
            *dependency_kind,
            nodes,
            walk_dependency_parameters,
        );

        all_out_text_tree_lines.append(&mut dep_kind_out);
    }

    all_out_text_tree_lines
}

fn construct_dependency_type_nodes_hashmap<'a>(
    graph: &'a Graph,
    package: &PackageId,
    print_config: &PrintConfig,
) -> HashMap<DependencyKind, Vec<PackageId>> {
    let mut dependency_type_nodes: HashMap<DependencyKind, Vec<PackageId>> = [
        (DependencyKind::Build, vec![]),
        (DependencyKind::Development, vec![]),
        (DependencyKind::Normal, vec![]),
    ]
    .iter()
    .cloned()
    .collect();

    for edge in graph
        .graph
        .edges_directed(graph.nodes[package], print_config.direction)
    {
        let dependency = match print_config.direction {
            EdgeDirection::Incoming => &graph.graph[edge.source()],
            EdgeDirection::Outgoing => &graph.graph[edge.target()],
        };

        dependency_type_nodes
            .get_mut(edge.weight())
            .unwrap()
            .push(dependency.clone());
    }

    dependency_type_nodes
}

#[cfg(test)]
mod dependency_node_tests {
    use super::*;

    use crate::format::pattern::Pattern;
    use crate::format::print_config::{OutputFormat, Prefix, PrintConfig};

    use krates::cm::DependencyKind;
    use geiger::IncludeTests;
    use petgraph::graph::NodeIndex;
    use rstest::*;

    #[rstest(
        input_directed_edges,
        input_edge_direction,
        expected_build_nodes_length,
        expected_development_nodes_length,
        expected_normal_nodes_length,
        case(
            vec![
                (1, 0, DependencyKind::Build),
                (2, 0, DependencyKind::Build),
                (3, 0, DependencyKind::Build),
                (4, 0, DependencyKind::Development),
                (5, 0, DependencyKind::Development),
                (6, 0, DependencyKind::Normal)
            ],
            EdgeDirection::Incoming,
            3,
            2,
            1
        ),
        case(
            vec![
                (0, 1, DependencyKind::Build),
                (0, 2, DependencyKind::Development),
                (0, 3, DependencyKind::Development),
                (0, 4, DependencyKind::Normal),
                (0, 5, DependencyKind::Normal),
                (0, 6, DependencyKind::Normal)
            ],
            EdgeDirection::Outgoing,
            1,
            2,
            3
        )
    )]
    fn construct_dependency_type_nodes_hashmap_test(
        input_directed_edges: Vec<(usize, usize, DependencyKind)>,
        input_edge_direction: EdgeDirection,
        expected_build_nodes_length: usize,
        expected_development_nodes_length: usize,
        expected_normal_nodes_length: usize,
    ) {
        let mut inner_graph =
            petgraph::Graph::<PackageId, DependencyKind>::new();
        let mut nodes = HashMap::<PackageId, NodeIndex>::new();

        let package_ids = create_cargo_metadata_package_id_vec(7);
        let print_config = create_print_config(input_edge_direction);

        for package_id in &package_ids {
            nodes.insert(
                package_id.clone(),
                inner_graph.add_node(package_id.clone()),
            );
        }

        add_edges_to_graph(
            &input_directed_edges,
            &mut inner_graph,
            &nodes,
            &package_ids,
        );

        let graph = Graph {
            graph: inner_graph,
            nodes,
        };

        let dependency_type_nodes_hashmap =
            construct_dependency_type_nodes_hashmap(
                &graph,
                &package_ids[0],
                &print_config,
            );

        assert_eq!(
            dependency_type_nodes_hashmap[&DependencyKind::Build].len(),
            expected_build_nodes_length
        );
        assert_eq!(
            dependency_type_nodes_hashmap[&DependencyKind::Development].len(),
            expected_development_nodes_length
        );
        assert_eq!(
            dependency_type_nodes_hashmap[&DependencyKind::Normal].len(),
            expected_normal_nodes_length
        );
    }

    fn add_edges_to_graph(
        directed_edges: &[(usize, usize, DependencyKind)],
        graph: &mut petgraph::Graph<PackageId, DependencyKind>,
        nodes: &HashMap<PackageId, NodeIndex>,
        package_ids: &[PackageId],
    ) {
        for (source_index, target_index, dep_kind) in directed_edges {
            graph.add_edge(
                nodes[&package_ids[*source_index]],
                nodes[&package_ids[*target_index]],
                *dep_kind,
            );
        }
    }

    fn create_cargo_metadata_package_id_vec(count: i32) -> Vec<PackageId> {
        (0..count)
            .map(|i| PackageId {
                repr: format!("string_repr_{}", i),
            })
            .collect()
    }

    fn create_print_config(edge_direction: EdgeDirection) -> PrintConfig {
        PrintConfig {
            all: false,
            allow_partial_results: false,
            direction: edge_direction,
            format: Pattern::new(vec![]),
            include_tests: IncludeTests::Yes,
            prefix: Prefix::Depth,
            output_format: OutputFormat::Ascii,
        }
    }
}
