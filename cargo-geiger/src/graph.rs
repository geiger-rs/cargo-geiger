use crate::args::{Args, DepsArgs, TargetArgs};
use crate::cli::get_cfgs;
use crate::mapping::{
    CargoMetadataParameters, DepsNotReplaced, MatchesIgnoringSource,
};

use cargo::core::Workspace;
use cargo::util::interning::InternedString;
use cargo::util::CargoResult;
use cargo::Config;
use cargo_metadata::{DependencyKind, PackageId};
use cargo_platform::{Cfg, Platform};
use petgraph::graph::NodeIndex;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub enum ExtraDeps {
    All,
    Build,
    Dev,
    NoMore,
}

impl ExtraDeps {
    // This clippy recommendation is valid, but makes this function much harder to read
    #[allow(clippy::match_like_matches_macro)]
    pub fn allows(&self, dep: DependencyKind) -> bool {
        match (self, dep) {
            (_, DependencyKind::Normal) => true,
            (ExtraDeps::All, _) => true,
            (ExtraDeps::Build, DependencyKind::Build) => true,
            (ExtraDeps::Dev, DependencyKind::Development) => true,
            _ => false,
        }
    }
}

/// Representation of the package dependency graph
pub struct Graph {
    pub graph: petgraph::Graph<PackageId, cargo_metadata::DependencyKind>,
    pub nodes: HashMap<PackageId, NodeIndex>,
}

// Almost unmodified compared to the original in cargo-tree, should be fairly
// simple to move this and the dependency graph structure out to a library.
/// Function to build a graph of packages dependencies
pub fn build_graph<'a>(
    args: &Args,
    cargo_metadata_parameters: &'a CargoMetadataParameters,
    config: &Config,
    root_package_id: PackageId,
    workspace: &Workspace,
) -> CargoResult<Graph> {
    let config_host = config.load_global_rustc(Some(&workspace))?.host;
    let (extra_deps, target) = build_graph_prerequisites(
        &config_host,
        &args.deps_args,
        &args.target_args,
    );
    let cfgs = get_cfgs(config, &args.target_args.target, &workspace)?;

    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };
    graph.nodes.insert(
        root_package_id.clone(),
        graph.graph.add_node(root_package_id.clone()),
    );

    let mut pending_packages = vec![root_package_id];

    let graph_configuration = GraphConfiguration {
        target,
        cfgs: cfgs.as_deref(),
        extra_deps,
    };

    while let Some(package_id) = pending_packages.pop() {
        add_package_dependencies_to_graph(
            cargo_metadata_parameters,
            package_id,
            &graph_configuration,
            &mut graph,
            &mut pending_packages,
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
    dependency: &cargo_metadata::Dependency,
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
) {
    let index = graph.nodes[&package_id];
    let package = cargo_metadata_parameters
        .krates
        .node_for_kid(&package_id)
        .unwrap()
        .krate
        .clone();

    for (dependency_package_id, _) in cargo_metadata_parameters
        .metadata
        .deps_not_replaced(package_id)
    {
        let dependency_iterator = package
            .dependencies
            .iter()
            .filter(|d| {
                d.matches_ignoring_source(
                    cargo_metadata_parameters.krates,
                    dependency_package_id.clone(),
                )
            })
            .filter(|d| graph_configuration.extra_deps.allows(d.kind))
            .filter(|d| {
                d.target
                    .as_ref()
                    .and_then(|p| {
                        graph_configuration.target.map(|t| {
                            match graph_configuration.cfgs {
                                None => false,
                                Some(cfgs) => {
                                    (Platform::from_str(p.repr.as_str()))
                                        .unwrap()
                                        .matches(t, cfgs)
                                }
                            }
                        })
                    })
                    .unwrap_or(true)
            });

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

fn build_graph_prerequisites<'a>(
    config_host: &'a InternedString,
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
        Some(target_args.target.as_deref().unwrap_or(&config_host))
    };

    (extra_deps, target)
}

#[cfg(test)]
mod graph_tests {
    use super::*;
    use rstest::*;

    #[rstest(
        input_extra_deps,
        input_dependency_kind,
        expected_allows,
        case(ExtraDeps::All, DependencyKind::Normal, true),
        case(ExtraDeps::Build, DependencyKind::Normal, true),
        case(ExtraDeps::Dev, DependencyKind::Normal, true),
        case(ExtraDeps::NoMore, DependencyKind::Normal, true),
        case(ExtraDeps::All, DependencyKind::Build, true),
        case(ExtraDeps::All, DependencyKind::Development, true),
        case(ExtraDeps::Build, DependencyKind::Build, true),
        case(ExtraDeps::Build, DependencyKind::Development, false),
        case(ExtraDeps::Dev, DependencyKind::Build, false),
        case(ExtraDeps::Dev, DependencyKind::Development, true)
    )]
    fn extra_deps_allows_test(
        input_extra_deps: ExtraDeps,
        input_dependency_kind: DependencyKind,
        expected_allows: bool,
    ) {
        assert_eq!(
            input_extra_deps.allows(input_dependency_kind),
            expected_allows
        );
    }

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
        let config_host = InternedString::new("config_host");
        let target_args = TargetArgs::default();

        let (extra_deps, _) = build_graph_prerequisites(
            &config_host,
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
        let config_host = InternedString::new("default_config_host");
        let deps_args = DepsArgs::default();

        let (_, target) = build_graph_prerequisites(
            &config_host,
            &deps_args,
            &input_target_args,
        );

        assert_eq!(target, expected_target);
    }
}
