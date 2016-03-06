extern crate cargo;
extern crate petgraph;
extern crate rustc_serialize;

use cargo::{Config, CliResult};
use cargo::core::{Source, PackageId, Package, Resolve};
use cargo::core::dependency::Kind;
use cargo::core::package::PackageSet;
use cargo::core::registry::PackageRegistry;
use cargo::core::resolver::Method;
use cargo::ops;
use cargo::util::{important_paths, CargoResult};
use cargo::sources::path::PathSource;
use std::collections::{HashSet, HashMap};
use petgraph::EdgeDirection;
use petgraph::graph::NodeIndex;

#[cfg_attr(rustfmt, rustfmt_skip)]
const USAGE: &'static str = "
Display a tree visualization of a dependency graph

Usage: cargo tree [options]
       cargo tree --help

Options:
    -h, --help              Print this message
    -V, --version           Print version info and exit
    -p, --package PACKAGE   Set the package to be used as the root of the tree
    -k, --kind KIND         Set the kind of dependencies to analyze. Valid
                            values: normal, dev, build [default: normal]
    --features FEATURES     Space separated list of features to include
    --no-default-features   Do not include the `default` feature
    --target TARGET         Set the target triple
    -i, --invert            Invert the tree direction
    --charset CHARSET       Set the character set to use in output. Valid
                            values: utf8, ascii [default: utf8]
    --manifest-path PATH    Path to the manifest to analyze
    -v, --verbose           Use verbose output
    -q, --quiet             No output printed to stdout other than the tree
";

#[derive(RustcDecodable)]
struct Flags {
    flag_version: bool,
    flag_package: Option<String>,
    flag_kind: RawKind,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_invert: bool,
    flag_charset: Charset,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_quiet: bool,
}

#[derive(RustcDecodable)]
enum Charset {
    Utf8,
    Ascii,
}

#[derive(RustcDecodable)]
enum RawKind {
    Normal,
    Dev,
    Build,
}

struct Symbols {
    down: &'static str,
    tee: &'static str,
    ell: &'static str,
    right: &'static str,
}

static UTF8_SYMBOLS: Symbols = Symbols {
    down: "│",
    tee: "├",
    ell: "└",
    right: "─",
};

static ASCII_SYMBOLS: Symbols = Symbols {
    down: "|",
    tee: "|",
    ell: "`",
    right: "-",
};

fn main() {
    cargo::execute_main_without_stdin(real_main, false, USAGE);
}

fn real_main(flags: Flags, config: &Config) -> CliResult<Option<()>> {
    let Flags { flag_version,
                flag_package,
                flag_kind,
                flag_features,
                flag_no_default_features,
                flag_target,
                flag_invert,
                flag_charset,
                flag_manifest_path,
                flag_verbose,
                flag_quiet } = flags;

    if flag_version {
        println!("cargo-tree {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }

    let flag_features = flag_features.iter()
                                     .flat_map(|s| s.split(" "))
                                     .map(|s| s.to_owned())
                                     .collect();

    try!(config.shell().set_verbosity(flag_verbose, flag_quiet));

    let mut source = try!(source(config, flag_manifest_path));
    let package = try!(source.root_package());
    let mut registry = try!(registry(config, &package));
    let resolve = try!(resolve(&mut registry,
                               &package,
                               flag_features,
                               flag_no_default_features));
    let packages = ops::get_resolved_packages(&resolve, registry);

    let root = match flag_package {
        Some(ref pkg) => try!(resolve.query(pkg)),
        None => resolve.root(),
    };

    let kind = match flag_kind {
        RawKind::Normal => Kind::Normal,
        RawKind::Dev => Kind::Development,
        RawKind::Build => Kind::Build,
    };

    let target = flag_target.as_ref().unwrap_or(&config.rustc_info().host);

    let graph = try!(build_graph(&resolve, &packages, package.package_id(), target));

    let direction = if flag_invert {
        EdgeDirection::Incoming
    } else {
        EdgeDirection::Outgoing
    };

    let symbols = match flag_charset {
        Charset::Ascii => &ASCII_SYMBOLS,
        Charset::Utf8 => &UTF8_SYMBOLS,
    };

    print_tree(root, kind, &graph, direction, symbols);

    Ok(None)
}

fn source(config: &Config, manifest_path: Option<String>) -> CargoResult<PathSource> {
    let root = try!(important_paths::find_root_manifest_for_wd(manifest_path, config.cwd()));
    let mut source = try!(PathSource::for_path(root.parent().unwrap(), config));
    try!(source.update());
    Ok(source)
}

fn registry<'a>(config: &'a Config, package: &Package) -> CargoResult<PackageRegistry<'a>> {
    let mut registry = PackageRegistry::new(config);
    try!(registry.add_sources(&[package.package_id().source_id().clone()]));
    Ok(registry)
}

fn resolve(registry: &mut PackageRegistry,
           package: &Package,
           features: Vec<String>,
           no_default_features: bool)
           -> CargoResult<Resolve> {
    let resolve = try!(ops::resolve_pkg(registry, package));

    let method = Method::Required {
        dev_deps: true,
        features: &features,
        uses_default_features: !no_default_features,
    };

    ops::resolve_with_previous(registry, &package, method, Some(&resolve), None)
}

struct Graph<'a> {
    graph: petgraph::Graph<&'a PackageId, Kind>,
    nodes: HashMap<&'a PackageId, NodeIndex>,
}

fn build_graph<'a>(resolve: &'a Resolve,
                   packages: &PackageSet,
                   root: &'a PackageId,
                   target: &str)
                   -> CargoResult<Graph<'a>> {
    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };
    graph.nodes.insert(root, graph.graph.add_node(root));

    let mut pending = vec![root];

    while let Some(pkg_id) = pending.pop() {
        let idx = graph.nodes[&pkg_id];
        let pkg = try!(packages.get(pkg_id));

        for dep_id in resolve.deps(pkg_id).unwrap() {
            for dep in pkg.dependencies()
                          .iter()
                          .filter(|d| d.matches_id(dep_id))
                          .filter(|d| {
                              d.platform().map(|p| p.matches(target, None)).unwrap_or(true)
                          }) {
                let dep_idx = {
                    let g = &mut graph.graph;
                    *graph.nodes.entry(dep_id).or_insert_with(|| g.add_node(dep_id))
                };
                graph.graph.update_edge(idx, dep_idx, dep.kind());
                pending.push(dep_id);
            }
        }
    }

    Ok(graph)
}

fn print_tree<'a>(package: &'a PackageId,
                  kind: Kind,
                  graph: &Graph<'a>,
                  direction: EdgeDirection,
                  symbols: &Symbols) {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];

    print_dependency(package,
                     kind,
                     &graph,
                     direction,
                     symbols,
                     &mut visited_deps,
                     &mut levels_continue);
}

fn print_dependency<'a>(package: &'a PackageId,
                        kind: Kind,
                        graph: &Graph<'a>,
                        direction: EdgeDirection,
                        symbols: &Symbols,
                        visited_deps: &mut HashSet<&'a PackageId>,
                        levels_continue: &mut Vec<bool>) {
    if let Some((&last_continues, rest)) = levels_continue.split_last() {
        for &continues in rest {
            let c = if continues {
                symbols.down
            } else {
                " "
            };
            print!("{}   ", c);
        }

        let c = if last_continues {
            symbols.tee
        } else {
            symbols.ell
        };
        print!("{0}{1}{1} ", c, symbols.right);
    }

    let new = visited_deps.insert(package);
    let star = if new {
        ""
    } else {
        " (*)"
    };

    println!("{}{}", package, star);

    if !new {
        return;
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    let mut deps = graph.graph
                        .edges_directed(graph.nodes[&package], direction)
                        .filter(|&(_, &k)| kind == k)
                        .map(|(i, _)| graph.graph[i])
                        .collect::<Vec<_>>();
    deps.sort();
    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        print_dependency(dependency,
                         Kind::Normal,
                         graph,
                         direction,
                         symbols,
                         visited_deps,
                         levels_continue);
        levels_continue.pop();
    }
}
