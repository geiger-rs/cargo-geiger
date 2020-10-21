use crate::format::print_config::PrintConfig;
use crate::graph::{Graph, Node};
use crate::tree::TextTreeLine;

use super::construct_tree_vines_string;
use super::walk_dependency_kind;

use cargo::core::dependency::DepKind;
use cargo::core::PackageId;
use petgraph::visit::EdgeRef;
use petgraph::EdgeDirection;
use std::collections::{HashMap, HashSet};

pub fn walk_dependency_node(
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

#[cfg(test)]
mod dependency_node_tests {
    use super::*;

    use crate::cli::get_workspace;
    use crate::format::pattern::Pattern;
    use crate::format::print_config::{Prefix, PrintConfig};
    use crate::format::Charset;

    use cargo::core::Verbosity;
    use cargo::Config;
    use geiger::IncludeTests;
    use petgraph::graph::NodeIndex;
    use rstest::*;
    use std::env;

    #[rstest(
        input_directed_edges,
        input_edge_direction,
        expected_build_nodes_length,
        expected_development_nodes_length,
        expected_normal_nodes_length,
        case(
            vec![
                (1, 0, DepKind::Build),
                (2, 0, DepKind::Build),
                (3, 0, DepKind::Build),
                (4, 0, DepKind::Development),
                (5, 0, DepKind::Development),
                (6, 0, DepKind::Normal)
            ],
            EdgeDirection::Incoming,
            3,
            2,
            1
        ),
        case(
            vec![
                (0, 1, DepKind::Build),
                (0, 2, DepKind::Development),
                (0, 3, DepKind::Development),
                (0, 4, DepKind::Normal),
                (0, 5, DepKind::Normal),
                (0, 6, DepKind::Normal)
            ],
            EdgeDirection::Outgoing,
            1,
            2,
            3
        )
    )]
    fn construct_dependency_type_nodes_hashmap_test(
        input_directed_edges: Vec<(usize, usize, DepKind)>,
        input_edge_direction: EdgeDirection,
        expected_build_nodes_length: usize,
        expected_development_nodes_length: usize,
        expected_normal_nodes_length: usize,
    ) {
        let mut inner_graph = petgraph::Graph::<Node, DepKind>::new();
        let mut nodes = HashMap::<PackageId, NodeIndex>::new();

        let package_ids = create_package_id_vec(7);
        let print_config = create_print_config(input_edge_direction);

        for package_id in &package_ids {
            nodes.insert(
                *package_id,
                inner_graph.add_node(Node { id: *package_id }),
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
                &Node { id: package_ids[0] },
                &print_config,
            );

        assert_eq!(
            dependency_type_nodes_hashmap[&DepKind::Build].len(),
            expected_build_nodes_length
        );
        assert_eq!(
            dependency_type_nodes_hashmap[&DepKind::Development].len(),
            expected_development_nodes_length
        );
        assert_eq!(
            dependency_type_nodes_hashmap[&DepKind::Normal].len(),
            expected_normal_nodes_length
        );
    }

    fn add_edges_to_graph(
        directed_edges: &[(usize, usize, DepKind)],
        graph: &mut petgraph::Graph<Node, DepKind>,
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

    fn create_package_id_vec(count: i32) -> Vec<PackageId> {
        let config = Config::default().unwrap();

        let current_working_dir =
            env::current_dir().unwrap().join("Cargo.toml");

        let manifest_path_option = Some(current_working_dir);

        let workspace = get_workspace(&config, manifest_path_option).unwrap();

        let package = workspace.current().unwrap();

        let source_id = package.dependencies().first().unwrap().source_id();

        let mut package_id_vec: Vec<PackageId> = vec![];

        for i in 0..count {
            package_id_vec.push(
                PackageId::new(
                    format!("test_name_{}", i),
                    format!("1.2.{}", i).as_str(),
                    source_id,
                )
                .unwrap(),
            )
        }

        package_id_vec
    }

    fn create_print_config(edge_direction: EdgeDirection) -> PrintConfig {
        PrintConfig {
            all: false,
            allow_partial_results: false,
            charset: Charset::Ascii,
            direction: edge_direction,
            format: Pattern(vec![]),
            include_tests: IncludeTests::Yes,
            prefix: Prefix::Depth,
            output_format: None,
            verbosity: Verbosity::Verbose,
        }
    }
}
