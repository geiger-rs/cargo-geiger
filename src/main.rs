extern crate cargo;
extern crate petgraph;
extern crate rustc_serialize;

use cargo::{Config, CliResult};
use cargo::core::{Source, PackageId, Package, Resolve};
use cargo::core::dependency::Kind;
use cargo::core::package::PackageSet;
use cargo::core::registry::PackageRegistry;
use cargo::core::resolver::Method;
use cargo::core::source::SourceId;
use cargo::core::manifest::ManifestMetadata;
use cargo::ops;
use cargo::util::{self, important_paths, CargoResult, Cfg};
use cargo::sources::path::PathSource;
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::Entry;
use std::str::{self, FromStr};
use petgraph::EdgeDirection;
use petgraph::graph::NodeIndex;

use format::Pattern;

mod format;

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
    --no-indent             Display dependencies as a list (rather than a graph)
    -a, --all               Don't truncate dependencies that have already been
                            displayed
    -d, --duplicates        Show only dependencies which come in multiple
                            versions (implies --invert)
    --charset CHARSET       Set the character set to use in output. Valid
                            values: utf8, ascii [default: utf8]
    -f, --format FORMAT     Format string for printing dependencies
    --manifest-path PATH    Path to the manifest to analyze
    -v, --verbose           Use verbose output
    -q, --quiet             No output printed to stdout other than the tree
    --color WHEN            Coloring: auto, always, never
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
    flag_no_indent: bool,
    flag_all: bool,
    flag_charset: Charset,
    flag_format: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: Option<bool>,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_duplicates: bool,
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
                flag_no_indent,
                flag_all,
                flag_charset,
                flag_format,
                flag_manifest_path,
                flag_verbose,
                flag_quiet,
                flag_color,
                flag_duplicates } = flags;

    if flag_version {
        println!("cargo-tree {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }

    let flag_features = flag_features.iter()
                                     .flat_map(|s| s.split(" "))
                                     .map(|s| s.to_owned())
                                     .collect();

    try!(config.configure_shell(flag_verbose, flag_quiet, &flag_color));

    let mut source = try!(source(config, flag_manifest_path));
    let package = try!(source.root_package());
    let mut registry = try!(registry(config, &package));
    let resolve = try!(resolve(&mut registry,
                               &package,
                               &config,
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

    let format = match flag_format {
        Some(ref r) => &**r,
        None => "{p}",
    };
    let format = try!(Pattern::new(format).map_err(cargo::human));

    let cfgs = try!(get_cfgs(config, &flag_target));
    let graph = try!(build_graph(&resolve,
                                 &packages,
                                 package.package_id(),
                                 target,
                                 cfgs.as_ref().map(|r| &**r)));

    let direction = if flag_invert || flag_duplicates {
        EdgeDirection::Incoming
    } else {
        EdgeDirection::Outgoing
    };

    let symbols = match flag_charset {
        Charset::Ascii => &ASCII_SYMBOLS,
        Charset::Utf8 => &UTF8_SYMBOLS,
    };

    if flag_duplicates {
        let dups = find_duplicates(&graph);
        for dup in &dups {
            print_tree(dup,
                       kind,
                       &graph,
                       &format,
                       direction,
                       symbols,
                       flag_no_indent,
                       flag_all);
            println!("");
        }
    } else {
        print_tree(root,
                   kind,
                   &graph,
                   &format,
                   direction,
                   symbols,
                   flag_no_indent,
                   flag_all);
    }

    Ok(None)
}

fn find_duplicates<'a>(graph: &Graph<'a>) -> Vec<&'a PackageId> {
    let mut counts = HashMap::new();

    // Count by name only. Source and version are irrelevant here.
    for package in graph.nodes.keys() {
        *counts.entry(package.name()).or_insert(0) += 1;
    }

    // Theoretically inefficient, but in practice we're only listing duplicates and
    // there won't be enough dependencies for it to matter.
    let mut dup_ids = Vec::new();
    for name in counts.drain().filter(|&(_, v)| v > 1).map(|(k, _)| k) {
        dup_ids.extend(graph.nodes.keys().filter(|p| p.name() == name));
    }
    dup_ids
}

fn get_cfgs(config: &Config, target: &Option<String>) -> CargoResult<Option<Vec<Cfg>>> {
    let mut process = util::process(config.rustc());
    process.arg("--print=cfg").env_remove("RUST_LOG");
    if let Some(ref s) = *target {
        process.arg("--target").arg(s);
    }

    let output = match process.exec_with_output() {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };
    let output = str::from_utf8(&output.stdout).unwrap();
    let lines = output.lines();
    Ok(Some(try!(lines.map(Cfg::from_str).collect())))
}

fn source(config: &Config, manifest_path: Option<String>) -> CargoResult<PathSource> {
    let root = try!(important_paths::find_root_manifest_for_wd(manifest_path, config.cwd()));
    let dir = root.parent().unwrap();
    let id = try!(SourceId::for_path(dir));
    let mut source = PathSource::new(dir, &id, config);
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
           config: &Config,
           features: Vec<String>,
           no_default_features: bool)
           -> CargoResult<Resolve> {
    let resolve = try!(ops::resolve_pkg(registry, package, config));

    let method = Method::Required {
        dev_deps: true,
        features: &features,
        uses_default_features: !no_default_features,
    };

    ops::resolve_with_previous(registry, &package, method, Some(&resolve), None)
}

struct Node<'a> {
    id: &'a PackageId,
    metadata: &'a ManifestMetadata,
}

struct Graph<'a> {
    graph: petgraph::Graph<Node<'a>, Kind>,
    nodes: HashMap<&'a PackageId, NodeIndex>,
}

fn build_graph<'a>(resolve: &'a Resolve,
                   packages: &'a PackageSet,
                   root: &'a PackageId,
                   target: &str,
                   cfgs: Option<&[Cfg]>)
                   -> CargoResult<Graph<'a>> {
    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };
    let node = Node {
        id: root,
        metadata: try!(packages.get(root)).manifest().metadata(),
    };
    graph.nodes.insert(root, graph.graph.add_node(node));

    let mut pending = vec![root];

    while let Some(pkg_id) = pending.pop() {
        let idx = graph.nodes[&pkg_id];
        let pkg = try!(packages.get(pkg_id));

        for dep_id in resolve.deps(pkg_id) {
            let it = pkg.dependencies()
                        .iter()
                        .filter(|d| d.matches_id(dep_id))
                        .filter(|d| d.platform().map(|p| p.matches(target, cfgs)).unwrap_or(true));
            for dep in it {
                let dep_idx = match graph.nodes.entry(dep_id) {
                    Entry::Occupied(e) => *e.get(),
                    Entry::Vacant(e) => {
                        pending.push(dep_id);
                        let node = Node {
                            id: dep_id,
                            metadata: try!(packages.get(dep_id)).manifest().metadata(),
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

fn print_tree<'a>(package: &'a PackageId,
                  kind: Kind,
                  graph: &Graph<'a>,
                  format: &Pattern,
                  direction: EdgeDirection,
                  symbols: &Symbols,
                  no_indent: bool,
                  all: bool) {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];

    let node = &graph.graph[graph.nodes[&package]];
    print_dependency(node,
                     kind,
                     &graph,
                     format,
                     direction,
                     symbols,
                     &mut visited_deps,
                     &mut levels_continue,
                     no_indent,
                     all);
}

fn print_dependency<'a>(package: &Node<'a>,
                        kind: Kind,
                        graph: &Graph<'a>,
                        format: &Pattern,
                        direction: EdgeDirection,
                        symbols: &Symbols,
                        visited_deps: &mut HashSet<&'a PackageId>,
                        levels_continue: &mut Vec<bool>,
                        no_indent: bool,
                        all: bool) {
    let new = all || visited_deps.insert(package.id);
    let star = if new {
        ""
    } else {
        " (*)"
    };

    if !no_indent {
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
    }

    println!("{}{}", format.display(package.id, package.metadata), star);

    if !new {
        return;
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    let mut deps = graph.graph
                        .edges_directed(graph.nodes[&package.id], direction)
                        .filter(|&(_, &k)| kind == k)
                        .map(|(i, _)| &graph.graph[i])
                        .collect::<Vec<_>>();
    deps.sort_by_key(|n| n.id);
    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        print_dependency(dependency,
                         Kind::Normal,
                         graph,
                         format,
                         direction,
                         symbols,
                         visited_deps,
                         levels_continue,
                         no_indent,
                         all);
        levels_continue.pop();
    }
}
