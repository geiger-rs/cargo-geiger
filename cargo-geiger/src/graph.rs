pub mod extra_deps;

use extra_deps::ExtraDeps;
use krates::Node;

use crate::args::{Args, DepsArgs, TargetArgs};
use crate::cli::get_cfgs;
use crate::mapping::{
    CargoMetadataParameters, DepsNotReplaced, MatchesIgnoringSource,
};

use cargo::util::CargoResult;
use cargo_metadata::{Dependency, DependencyKind, Package, PackageId};
use cargo_platform::Cfg;
use petgraph::graph::NodeIndex;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;

/// Representation of the package dependency graph
pub struct Graph {
    pub graph: petgraph::Graph<PackageId, DependencyKind>,
    pub nodes: HashMap<PackageId, NodeIndex>,
}

// Almost unmodified compared to the original in cargo-tree, should be fairly
// simple to move this and the dependency graph structure out to a library.
/// Function to build a graph of packages dependencies
pub fn build_graph<'a>(
    args: &Args,
    cargo_metadata_parameters: &'a CargoMetadataParameters,
    config_host: &'a str,
    global_rustc_path: &'a PathBuf,
    root_package_id: PackageId,
) -> CargoResult<Graph> {
    let (extra_deps, target) = build_graph_prerequisites(
        config_host,
        &args.deps_args,
        &args.target_args,
    );
    let cfgs = get_cfgs(global_rustc_path, &args.target_args.target)?;

    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };
    graph.nodes.insert(
        root_package_id.clone(),
        graph.graph.add_node(root_package_id.clone()),
    );

    let mut pending_packages = vec![root_package_id.clone()];

    let graph_configuration = GraphConfiguration {
        target,
        cfgs: cfgs.as_deref(),
        extra_deps,
    };

    while let Some(package_id) = pending_packages.pop() {
        let is_root_package = package_id == root_package_id;
        add_package_dependencies_to_graph(
            cargo_metadata_parameters,
            package_id,
            &graph_configuration,
            &mut graph,
            &mut pending_packages,
            is_root_package,
        );
    }

    Ok(graph)
}

struct GraphConfiguration<'a> {
    target: Option<&'a str>,
    cfgs: Option<&'a [Cfg]>,
    extra_deps: ExtraDeps,
}

fn add_graph_node_if_not_present_and_edge(
    dependency: &Dependency,
    dependency_package_id: PackageId,
    graph: &mut Graph,
    index: NodeIndex,
    pending_packages: &mut Vec<PackageId>,
) {
    let dependency_index =
        match graph.nodes.entry(dependency_package_id.clone()) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                pending_packages.push(dependency_package_id.clone());
                *e.insert(graph.graph.add_node(dependency_package_id))
            }
        };
    graph
        .graph
        .add_edge(index, dependency_index, dependency.kind);
}

fn add_package_dependencies_to_graph(
    cargo_metadata_parameters: &CargoMetadataParameters,
    package_id: PackageId,
    graph_configuration: &GraphConfiguration,
    graph: &mut Graph,
    pending_packages: &mut Vec<PackageId>,
    is_root_package: bool,
) {
    let index = graph.nodes[&package_id];

    let krates_node_option =
        cargo_metadata_parameters.krates.node_for_kid(&package_id);

    let dep_not_replaced_option = cargo_metadata_parameters
        .metadata
        .deps_not_replaced(&package_id, is_root_package);

    match (krates_node_option, dep_not_replaced_option) {
        (Some(Node::<Package>::Krate { krate, .. }), Some(dependencies)) => {
            let package = krate.clone();

            for (dependency_package_id, _) in dependencies {
                let dependency_iterator = filter_dependencies(
                    cargo_metadata_parameters,
                    &dependency_package_id,
                    graph_configuration,
                    &package,
                );

                for dependency in dependency_iterator {
                    add_graph_node_if_not_present_and_edge(
                        dependency,
                        dependency_package_id.clone(),
                        graph,
                        index,
                        pending_packages,
                    );
                }
            }
        }
        _ => {
            eprintln!("Failed to add package dependencies to graph for Package Id: {}", package_id)
        }
    }
}

fn build_graph_prerequisites<'a>(
    config_host: &'a str,
    deps_args: &'a DepsArgs,
    target_args: &'a TargetArgs,
) -> (ExtraDeps, Option<&'a str>) {
    let extra_deps = if deps_args.all_deps {
        ExtraDeps::All
    } else if deps_args.build_deps {
        ExtraDeps::Build
    } else if deps_args.dev_deps {
        ExtraDeps::Dev
    } else {
        ExtraDeps::NoMore
    };

    let target = if target_args.all_targets {
        None
    } else {
        Some(target_args.target.as_deref().unwrap_or(config_host))
    };

    (extra_deps, target)
}

fn filter_dependencies<'a>(
    cargo_metadata_parameters: &'a CargoMetadataParameters,
    dependency_package_id: &'a PackageId,
    graph_configuration: &'a GraphConfiguration,
    package: &'a Package,
) -> Vec<&'a Dependency> {
    package
        .dependencies
        .iter()
        .filter(|d| {
            d.matches_ignoring_source(
                cargo_metadata_parameters.krates,
                dependency_package_id,
            )
            .unwrap_or(false)
        })
        .filter(|d| graph_configuration.extra_deps.allows(d.kind))
        .filter(|d| {
            d.target
                .as_ref()
                .and_then(|p| {
                    graph_configuration.target.map(
                        |t| match graph_configuration.cfgs {
                            None => false,
                            Some(cfgs) => p.matches(t, cfgs),
                        },
                    )
                })
                .unwrap_or(true)
        })
        .collect::<Vec<&Dependency>>()
}

#[cfg(test)]
mod graph_tests {
    use super::*;
    use rstest::*;

    #[rstest(
        input_deps_args,
        expected_extra_deps,
        case(
            DepsArgs {
                all_deps: true,
                build_deps: false,
                dev_deps: false
            },
            ExtraDeps::All
        ),
        case(
            DepsArgs {
                all_deps: false,
                build_deps: true,
                dev_deps: false
            },
            ExtraDeps::Build
        ),
        case(
            DepsArgs {
                all_deps: false,
                build_deps: false,
                dev_deps: true
            },
            ExtraDeps::Dev
        ),
        case(
            DepsArgs {
                all_deps: false,
                build_deps: false,
                dev_deps: false
            },
            ExtraDeps::NoMore
        )
    )]
    fn build_graph_prerequisites_extra_deps_test(
        input_deps_args: DepsArgs,
        expected_extra_deps: ExtraDeps,
    ) {
        let config_host = "config_host";
        let target_args = TargetArgs::default();

        let (extra_deps, _) = build_graph_prerequisites(
            config_host,
            &input_deps_args,
            &target_args,
        );

        assert_eq!(extra_deps, expected_extra_deps);
    }

    #[rstest(
        input_target_args,
        expected_target,
        case(
            TargetArgs {
                all_targets: true,
                target: None
            },
            None
        ),
        case(
            TargetArgs {
                all_targets: false,
                target: None
            },
            Some("default_config_host")),
        case(
            TargetArgs {
                all_targets: false,
                target: Some(String::from("provided_config_host")),
            },
            Some("provided_config_host")
        )
    )]
    fn build_graph_prerequisites_all_targets_test(
        input_target_args: TargetArgs,
        expected_target: Option<&str>,
    ) {
        let config_host = "default_config_host";
        let deps_args = DepsArgs::default();

        let (_, target) = build_graph_prerequisites(
            config_host,
            &deps_args,
            &input_target_args,
        );

        assert_eq!(target, expected_target);
    }
}
