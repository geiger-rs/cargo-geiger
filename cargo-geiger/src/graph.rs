use cargo::core::{PackageId, Resolve};
use cargo::core::dependency::DepKind;
use cargo::core::package::PackageSet;
use cargo::util::CargoResult;
use cargo_platform::Cfg;
use petgraph::graph::NodeIndex;
use std::collections::hash_map::Entry;
use std::collections::{HashMap};

// ---------- BEGIN: Public items ----------

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

pub struct Graph {
    pub graph: petgraph::Graph<Node, DepKind>,
    pub nodes: HashMap<PackageId, NodeIndex>,
}

pub struct Node {
    pub id: PackageId,
    // TODO: Investigate why this was needed before the separation of printing
    // and graph traversal and if it should be added back.
    //pack: &'a Package,
}

/// Almost unmodified compared to the original in cargo-tree, should be fairly
/// simple to move this and the dependency graph structure out to a library.
pub fn build_graph<'a>(
    resolve: &'a Resolve,
    packages: &'a PackageSet,
    root: PackageId,
    target: Option<&str>,
    cfgs: Option<&[Cfg]>,
    extra_deps: ExtraDeps,
) -> CargoResult<Graph> {
    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };
    let node = Node {
        id: root,
        //pack: packages.get_one(root)?,
    };
    graph.nodes.insert(root, graph.graph.add_node(node));

    let mut pending = vec![root];

    while let Some(pkg_id) = pending.pop() {
        let idx = graph.nodes[&pkg_id];
        let pkg = packages.get_one(pkg_id)?;

        for raw_dep_id in resolve.deps_not_replaced(pkg_id) {
            let it = pkg
                .dependencies()
                .iter()
                .filter(|d| d.matches_ignoring_source(raw_dep_id.0))
                .filter(|d| extra_deps.allows(d.kind()))
                .filter(|d| {
                    d.platform()
                        .and_then(|p| {
                            target.map(|t| match cfgs {
                                None => false,
                                Some(cfgs) => p.matches(t, cfgs),
                            })
                        })
                        .unwrap_or(true)
                });
            let dep_id = match resolve.replacement(raw_dep_id.0) {
                Some(id) => id,
                None => raw_dep_id.0,
            };
            for dep in it {
                let dep_idx = match graph.nodes.entry(dep_id) {
                    Entry::Occupied(e) => *e.get(),
                    Entry::Vacant(e) => {
                        pending.push(dep_id);
                        let node = Node {
                            id: dep_id,
                            //pack: packages.get_one(dep_id)?,
                        };
                        *e.insert(graph.graph.add_node(node))
                    }
                };
                graph.graph.add_edge(idx, dep_idx, dep.kind());
            }
        }
    }

    Ok(graph)
}

// ---------- END: Public items ----------
