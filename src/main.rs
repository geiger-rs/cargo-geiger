extern crate cargo;
extern crate petgraph;
extern crate rustc_serialize;

use cargo::{Config, CliResult};
use cargo::core::{Source, PackageId, Resolve};
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
    -p, --package PACKAGE   Set the package to be used as the root of the tree
    --features FEATURES     Space separated list of features to include
    --no-default-features   Do not include the `default` feature
    -i, --invert            Invert the tree direction
    --charset CHARSET       Set the character set to use in output. Valid
                            values: UTF8, ASCII [default: UTF8]
    --manifest-path PATH    Path to the manifest to analyze
    -v, --verbose           Use verbose output
    -q, --quiet             No output printed to stdout other than the tree
";

#[derive(RustcDecodable)]
struct Flags {
    flag_package: Option<String>,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
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
    let Flags {
        flag_package,
        flag_features,
        flag_no_default_features,
        flag_invert,
        flag_charset,
        flag_manifest_path,
        flag_verbose,
        flag_quiet,
    } = flags;

    try!(config.shell().set_verbosity(flag_verbose, flag_quiet));

    let resolve = try!(resolve(config,
                               flag_features,
                               flag_no_default_features,
                               flag_manifest_path));

    let root = match flag_package {
        Some(ref pkg) => try!(resolve.query(pkg)),
        None => resolve.root(),
    };

    let graph = build_graph(&resolve);

    let direction = if flag_invert {
        EdgeDirection::Incoming
    } else {
        EdgeDirection::Outgoing
    };

    let symbols = match flag_charset {
        Charset::Ascii => &ASCII_SYMBOLS,
        Charset::Utf8 => &UTF8_SYMBOLS,
    };

    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];

    print_dependency(root,
                     &graph,
                     direction,
                     symbols,
                     &mut visited_deps,
                     &mut levels_continue);

    Ok(None)
}

fn resolve(config: &Config,
           features: Vec<String>,
           no_default_features: bool,
           manifest_path: Option<String>) -> CargoResult<Resolve> {
    // Load the root package
    let root = try!(important_paths::find_root_manifest_for_cwd(manifest_path));
    let mut source = try!(PathSource::for_path(root.parent().unwrap(), config));
    try!(source.update());
    let package = try!(source.root_package());

    // Resolve all dependencies (generating or using Cargo.lock if necessary)
    let mut registry = PackageRegistry::new(config);
    try!(registry.add_sources(&[package.package_id().source_id().clone()]));
    let resolve = try!(ops::resolve_pkg(&mut registry, &package));

    let method = Method::Required {
        dev_deps: true,
        features: &features,
        uses_default_features: !no_default_features,
    };

    ops::resolve_with_previous(&mut registry, &package, method, Some(&resolve), None)
}

struct Graph<'a> {
    graph: petgraph::Graph<&'a PackageId, ()>,
    nodes: HashMap<&'a PackageId, NodeIndex>,
}

fn build_graph<'a>(resolve: &'a Resolve) -> Graph<'a> {
    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };

    for pkg in resolve.iter() {
        let idx = graph.graph.add_node(pkg);
        graph.nodes.insert(pkg, idx);
    }

    for pkg in resolve.iter() {
        for dep in resolve.deps(pkg).unwrap() {
            graph.graph.add_edge(graph.nodes[&pkg], graph.nodes[&dep], ());
        }
    }

    graph
}

fn print_dependency<'a>(package: &'a PackageId,
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
            print!("{}  ", c);
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
    let mut deps = graph.graph.neighbors_directed(graph.nodes[&package], direction)
                              .map(|i| graph.graph[i])
                              .collect::<Vec<_>>();
    deps.sort();
    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        print_dependency(dependency, graph, direction, symbols, visited_deps, levels_continue);
        levels_continue.pop();
    }
}
