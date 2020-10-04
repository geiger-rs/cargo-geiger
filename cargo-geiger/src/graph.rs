use crate::args::Args;
use crate::cli::get_cfgs;

use cargo::core::dependency::DepKind;
use cargo::core::package::PackageSet;
use cargo::core::{Dependency, PackageId, Resolve, Workspace};
use cargo::util::interning::InternedString;
use cargo::util::CargoResult;
use cargo::Config;
use cargo_platform::Cfg;
use petgraph::graph::NodeIndex;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub enum ExtraDeps {
    All,
    Build,
    Dev,
    NoMore,
}

impl ExtraDeps {
    pub fn allows(&self, dep: DepKind) -> bool {
        match (self, dep) {
            (_, DepKind::Normal) => true,
            (ExtraDeps::All, _) => true,
            (ExtraDeps::Build, DepKind::Build) => true,
            (ExtraDeps::Dev, DepKind::Development) => true,
            _ => false,
        }
    }
}

/// Representation of the package dependency graph
pub struct Graph {
    pub graph: petgraph::Graph<Node, DepKind>,
    pub nodes: HashMap<PackageId, NodeIndex>,
}

/// Representation of a node within the package dependency graph
pub struct Node {
    pub id: PackageId,
    // TODO: Investigate why this was needed before the separation of printing
    // and graph traversal and if it should be added back.
    //pack: &'a Package,
}

// Almost unmodified compared to the original in cargo-tree, should be fairly
// simple to move this and the dependency graph structure out to a library.
/// Function to build a graph of packages dependencies
pub fn build_graph<'a>(
    args: &Args,
    config: &Config,
    resolve: &'a Resolve,
    package_set: &'a PackageSet,
    root_package_id: PackageId,
    workspace: &Workspace,
) -> CargoResult<Graph> {
    let config_host = config.load_global_rustc(Some(&workspace))?.host;
    let (extra_deps, target) = build_graph_prerequisites(args, &config_host)?;
    let cfgs = get_cfgs(config, &args.target, &workspace)?;

    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };
    let node = Node {
        id: root_package_id,
        //pack: packages.get_one(root)?,
    };
    graph
        .nodes
        .insert(root_package_id, graph.graph.add_node(node));

    let mut pending_packages = vec![root_package_id];

    let graph_configuration = GraphConfiguration {
        target,
        cfgs: cfgs.as_deref(),
        extra_deps,
    };

    while let Some(package_id) = pending_packages.pop() {
        add_package_dependencies_to_graph(
            resolve,
            package_id,
            package_set,
            &graph_configuration,
            &mut graph,
            &mut pending_packages,
        )?;
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
    let dependency_index = match graph.nodes.entry(dependency_package_id) {
        Entry::Occupied(e) => *e.get(),
        Entry::Vacant(e) => {
            pending_packages.push(dependency_package_id);
            let node = Node {
                id: dependency_package_id,
                //pack: packages.get_one(dep_id)?,
            };
            *e.insert(graph.graph.add_node(node))
        }
    };
    graph
        .graph
        .add_edge(index, dependency_index, dependency.kind());
}

#[doc(hidden)]
fn add_package_dependencies_to_graph<'a>(
    resolve: &'a Resolve,
    package_id: PackageId,
    package_set: &'a PackageSet,
    graph_configuration: &GraphConfiguration,
    graph: &mut Graph,
    pending_packages: &mut Vec<PackageId>,
) -> CargoResult<()> {
    let index = graph.nodes[&package_id];
    let package = package_set.get_one(package_id)?;

    for (raw_dependency_package_id, _) in resolve.deps_not_replaced(package_id)
    {
        let dependency_iterator = package
            .dependencies()
            .iter()
            .filter(|d| d.matches_ignoring_source(raw_dependency_package_id))
            .filter(|d| graph_configuration.extra_deps.allows(d.kind()))
            .filter(|d| {
                d.platform()
                    .and_then(|p| {
                        graph_configuration.target.map(|t| {
                            match graph_configuration.cfgs {
                                None => false,
                                Some(cfgs) => p.matches(t, cfgs),
                            }
                        })
                    })
                    .unwrap_or(true)
            });

        let dependency_package_id =
            match resolve.replacement(raw_dependency_package_id) {
                Some(id) => id,
                None => raw_dependency_package_id,
            };

        for dependency in dependency_iterator {
            add_graph_node_if_not_present_and_edge(
                dependency,
                dependency_package_id,
                graph,
                index,
                pending_packages,
            );
        }
    }

    Ok(())
}

fn build_graph_prerequisites<'a>(
    args: &'a Args,
    config_host: &'a InternedString,
) -> CargoResult<(ExtraDeps, Option<&'a str>)> {
    let extra_deps = if args.all_deps {
        ExtraDeps::All
    } else if args.build_deps {
        ExtraDeps::Build
    } else if args.dev_deps {
        ExtraDeps::Dev
    } else {
        ExtraDeps::NoMore
    };

    let target = if args.all_targets {
        None
    } else {
        Some(args.target.as_deref().unwrap_or(&config_host))
    };

    Ok((extra_deps, target))
}

#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn extra_deps_allows_test() {
        assert_eq!(ExtraDeps::All.allows(DepKind::Normal), true);
        assert_eq!(ExtraDeps::Build.allows(DepKind::Normal), true);
        assert_eq!(ExtraDeps::Dev.allows(DepKind::Normal), true);
        assert_eq!(ExtraDeps::NoMore.allows(DepKind::Normal), true);

        assert_eq!(ExtraDeps::All.allows(DepKind::Build), true);
        assert_eq!(ExtraDeps::All.allows(DepKind::Development), true);

        assert_eq!(ExtraDeps::Build.allows(DepKind::Build), true);
        assert_eq!(ExtraDeps::Build.allows(DepKind::Development), false);

        assert_eq!(ExtraDeps::Dev.allows(DepKind::Build), false);
        assert_eq!(ExtraDeps::Dev.allows(DepKind::Development), true);
    }
}
