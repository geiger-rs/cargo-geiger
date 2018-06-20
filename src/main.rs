extern crate syn;
extern crate walkdir;

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use self::walkdir::WalkDir;

use syn::{visit, ItemFn, Expr, ItemImpl, ItemTrait, ImplItemMethod};

unsafe fn foo() {
    unsafe {
        let a = 10;
        println!("Bar");
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Count {
    pub num: u64,
    pub unsafe_num: u64,
}

impl Count {
    fn count(&mut self, is_unsafe: bool) {
        self.num += 1;
        if is_unsafe {
            self.unsafe_num += 1
        }
    }
}

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.unsafe_num, self.num)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct UnsafeCounter {
    pub functions: Count,

    pub exprs: Count,

    pub itemimpls: Count,

    pub itemtraits: Count,

    pub methods: Count,

    in_unsafe_block: bool,
}

impl<'ast> visit::Visit<'ast> for UnsafeCounter {
    fn visit_item_fn(&mut self, i: &ItemFn) {
        // fn definitions
        self.functions.count(i.unsafety.is_some());
        visit::visit_item_fn(self, i);
    }

    fn visit_expr(&mut self, i: &Expr) {
        // Total number of expressions of any type
        match i {
            Expr::Unsafe(i) => {
                self.in_unsafe_block = true;
                visit::visit_expr_unsafe(self, i);
                self.in_unsafe_block = false;
            }
            Expr::Path(_) | Expr::Lit(_) => {
                // Do not count. The expression `f(x)` should count as one
                // expression, not three.
            }
            other => {
                self.exprs.count(self.in_unsafe_block);
                visit::visit_expr(self, other);
            }
        }
    }

    fn visit_item_impl(&mut self, i: &ItemImpl) {
        // unsafe trait impl's
        self.itemimpls.count(i.unsafety.is_some());
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, i: &ItemTrait) {
        // Unsafe traits
        self.itemtraits.count(i.unsafety.is_some());
        visit::visit_item_trait(self, i);

    }

    fn visit_impl_item_method(&mut self, i: &ImplItemMethod) {
        self.methods.count(i.sig.unsafety.is_some());
        visit::visit_impl_item_method(self, i);
    }
}

impl fmt::Display for UnsafeCounter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Unsafe functions: {}", self.functions)?;
        writeln!(f, "Unsafe expressions: {}", self.exprs)?;
        writeln!(f, "Unsafe traits: {}", self.itemtraits)?;
        writeln!(f, "Unsafe methods: {}", self.methods)?;
        write!(f, "Unsafe impls: {}", self.itemimpls)
    }
}

pub fn find_unsafe(p: &Path, allow_partial_results: bool) -> UnsafeCounter {
    let counters = &mut UnsafeCounter::default();
    let walker = WalkDir::new(p).into_iter();
    for entry in walker {
        let entry = entry.expect("walkdir error, TODO: Implement error handling");
        if !entry.file_type().is_file() {
            // TODO: Add --verbose flag and proper logging.
            // println!("Skipping non-file: {}", p.display());
            continue;
        }
        let p = entry.path();
        let ext = match p.extension() {
            Some(e) => e,
            None    => continue
        };
        // to_string_lossy is ok since we only want to match against an ASCII
        // compatible extension and we do not keep the possibly lossy result
        // around.
        if ext.to_string_lossy() != "rs" {
            // TODO: Add --verbose flag and proper logging.
            // println!("Skipping non-rust: {}", p.display());
            continue;
        }
        // TODO: Add --verbose flag and proper logging.
        // println!("Processing file {}", p.display());
        let mut file = File::open(p).expect("Unable to open file");
        let mut src = String::new();
        file.read_to_string(&mut src).expect("Unable to read file");
        let syntax = match (allow_partial_results, syn::parse_file(&src)) {
            (_, Ok(s)) => s,
            (true, Err(e)) => {
                // TODO: Do proper error logging.
                println!("Failed to parse file: {}, {:?}", p.display(), e);
                continue
            },
            (false, Err(e)) => panic!("Failed to parse file: {}, {:?} ", p.display(), e)
        };
        syn::visit::visit_file(counters, &syntax);
    }
    *counters
}



// The code below is based on the source from cargo-tree.
// There is a whole lot of code that could be deleted or moved to a library
// used by both cargo-tree and this project.
// TODO: Move this and the unsafe_finder code to two modules after a
// hypothetical PR merge.

extern crate cargo;
extern crate env_logger;
extern crate failure;
extern crate petgraph;
extern crate colored;

#[macro_use]
extern crate structopt;

use cargo::core::dependency::Kind;
use cargo::core::package::PackageSet;
use cargo::core::registry::PackageRegistry;
use cargo::core::resolver::Method;
use cargo::core::shell::Shell;
use cargo::core::{Package, PackageId, Resolve, Workspace};
use cargo::ops;
use cargo::util::{self, important_paths, CargoResult, Cfg};
use cargo::{CliResult, Config};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::EdgeDirection;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::str::{self, FromStr};
use structopt::clap::AppSettings;
use structopt::StructOpt;

use format::Pattern;

mod format;

use colored::*;

#[derive(StructOpt)]
#[structopt(bin_name = "cargo")]
enum Opts {
    #[structopt(
        name = "osha",
        raw(
            setting = "AppSettings::UnifiedHelpMessage",
            setting = "AppSettings::DeriveDisplayOrder",
            setting = "AppSettings::DontCollapseArgsInUsage"
        )
    )]
    /// Display a tree visualization of a dependency graph
    Tree(Args),
}

#[derive(StructOpt)]
struct Args {
    #[structopt(long = "package", short = "p", value_name = "SPEC")]
    /// Package to be used as the root of the tree
    package: Option<String>,
    #[structopt(long = "features", value_name = "FEATURES")]
    /// Space-separated list of features to activate
    features: Option<String>,
    #[structopt(long = "all-features")]
    /// Activate all available features
    all_features: bool,
    #[structopt(long = "no-default-features")]
    /// Do not activate the `default` feature
    no_default_features: bool,
    #[structopt(long = "target", value_name = "TARGET")]
    /// Set the target triple
    target: Option<String>,
    #[structopt(long = "all-targets")]
    /// Return dependencies for all targets. By default only the host target is matched.
    all_targets: bool,
    #[structopt(long = "manifest-path", value_name = "PATH", parse(from_os_str))]
    /// Path to Cargo.toml
    manifest_path: Option<PathBuf>,
    #[structopt(long = "invert", short = "i")]
    /// Invert the tree direction
    invert: bool,
    #[structopt(long = "no-indent")]
    /// Display the dependencies as a list (rather than a tree)
    no_indent: bool,
    #[structopt(long = "prefix-depth")]
    /// Display the dependencies as a list (rather than a tree), but prefixed with the depth
    prefix_depth: bool,
    #[structopt(long = "all", short = "a")]
    /// Don't truncate dependencies that have already been displayed
    all: bool,
    #[structopt(long = "duplicate", short = "d")]
    /// Show only dependencies which come in multiple versions (implies -i)
    duplicates: bool,
    #[structopt(long = "charset", value_name = "CHARSET", default_value = "utf8")]
    /// Character set to use in output: utf8, ascii
    charset: Charset,
    #[structopt(long = "format", short = "f", value_name = "FORMAT", default_value = "{p}")]
    /// Format string used for printing dependencies
    format: String,
    #[structopt(long = "verbose", short = "v", parse(from_occurrences))]
    /// Use verbose output (-vv very verbose/build.rs output)
    verbose: u32,
    #[structopt(long = "quiet", short = "q")]
    /// No output printed to stdout other than the tree
    quiet: Option<bool>,
    #[structopt(long = "color", value_name = "WHEN")]
    /// Coloring: auto, always, never
    color: Option<String>,
    #[structopt(long = "frozen")]
    /// Require Cargo.lock and cache are up to date
    frozen: bool,
    #[structopt(long = "locked")]
    /// Require Cargo.lock is up to date
    locked: bool,
    #[structopt(short = "Z", value_name = "FLAG")]
    /// Unstable (nightly-only) flags to Cargo
    unstable_flags: Vec<String>,
}

enum Charset {
    Utf8,
    Ascii,
}

#[derive(Clone, Copy)]
enum Prefix {
    None,
    Indent,
    Depth,
}

impl FromStr for Charset {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Charset, &'static str> {
        match s {
            "utf8" => Ok(Charset::Utf8),
            "ascii" => Ok(Charset::Ascii),
            _ => Err("invalid charset"),
        }
    }
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
    env_logger::init();

    let mut config = match Config::default() {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e.into(), &mut shell)
        }
    };

    let Opts::Tree(args) = Opts::from_args();

    if let Err(e) = real_main(args, &mut config) {
        let mut shell = Shell::new();
        cargo::exit_with_error(e.into(), &mut shell)
    }
}

fn real_main(args: Args, config: &mut Config) -> CliResult {
    config.configure(
        args.verbose,
        args.quiet,
        &args.color,
        args.frozen,
        args.locked,
        &args.unstable_flags,
    )?;

    let workspace = workspace(config, args.manifest_path)?;
    let package = workspace.current()?;
    let mut registry = registry(config, &package)?;
    let (packages, resolve) = resolve(
        &mut registry,
        &workspace,
        args.features,
        args.all_features,
        args.no_default_features,
    )?;
    let ids = packages.package_ids().cloned().collect::<Vec<_>>();
    let packages = registry.get(&ids);

    let root = match args.package {
        Some(ref pkg) => resolve.query(pkg)?,
        None => package.package_id(),
    };

    let target = if args.all_targets {
        None
    } else {
        Some(args.target.as_ref().unwrap_or(&config.rustc()?.host).as_str())
    };

    let format = Pattern::new(&args.format).map_err(|e| failure::err_msg(e.to_string()))?;

    let cfgs = get_cfgs(config, &args.target)?;
    let graph = build_graph(
        &resolve,
        &packages,
        package.package_id(),
        target,
        cfgs.as_ref().map(|r| &**r),
    )?;

    let direction = if args.invert || args.duplicates {
        EdgeDirection::Incoming
    } else {
        EdgeDirection::Outgoing
    };

    let symbols = match args.charset {
        Charset::Ascii => &ASCII_SYMBOLS,
        Charset::Utf8 => &UTF8_SYMBOLS,
    };

    let prefix = if args.prefix_depth {
        Prefix::Depth
    } else if args.no_indent {
        Prefix::None
    } else {
        Prefix::Indent
    };

    println!();
    println!("{}", "Compact unsafe info: (functions, expressions, impls, traits, methods)".bold());
    println!();

    if args.duplicates {
        let dups = find_duplicates(&graph);
        for dup in &dups {
            print_tree(
                dup,
                &graph,
                &format,
                direction,
                symbols,
                prefix,
                args.all,
            );
            println!();
        }
    } else {
        print_tree(
            root,
            &graph,
            &format,
            direction,
            symbols,
            prefix,
            args.all,
        );
    }

    Ok(())
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
    dup_ids.sort();
    dup_ids
}

fn get_cfgs(config: &Config, target: &Option<String>) -> CargoResult<Option<Vec<Cfg>>> {
    let mut process = util::process(&config.rustc()?.path);
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
    Ok(Some(lines
        .map(Cfg::from_str)
        .collect::<CargoResult<Vec<_>>>()?))
}

fn workspace(config: &Config, manifest_path: Option<PathBuf>) -> CargoResult<Workspace> {
    let root = match manifest_path {
        Some(path) => path,
        None => important_paths::find_root_manifest_for_wd(config.cwd())?,
    };
    Workspace::new(&root, config)
}

fn registry<'a>(config: &'a Config, package: &Package) -> CargoResult<PackageRegistry<'a>> {
    let mut registry = PackageRegistry::new(config)?;
    registry.add_sources(&[package.package_id().source_id().clone()])?;
    Ok(registry)
}

fn resolve<'a, 'cfg>(
    registry: &mut PackageRegistry<'cfg>,
    workspace: &'a Workspace<'cfg>,
    features: Option<String>,
    all_features: bool,
    no_default_features: bool,
) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let features = Method::split_features(&features.into_iter().collect::<Vec<_>>());

    let (packages, resolve) = ops::resolve_ws(workspace)?;

    let method = Method::Required {
        dev_deps: true,
        features: &features,
        all_features,
        uses_default_features: !no_default_features,
    };

    let resolve = ops::resolve_with_previous(
        registry,
        workspace,
        method,
        Some(&resolve),
        None,
        &[],
        true,
        true,
    )?;
    Ok((packages, resolve))
}

struct Node<'a> {
    id: &'a PackageId,
    pack: &'a Package,
}

struct Graph<'a> {
    graph: petgraph::Graph<Node<'a>, Kind>,
    nodes: HashMap<&'a PackageId, NodeIndex>,
}

/// Almost unmodified compared to the original in cargo-tree, should be fairly
/// simple to move this and the dependency graph structure out to a library.
fn build_graph<'a>(
    resolve: &'a Resolve,
    packages: &'a PackageSet,
    root: &'a PackageId,
    target: Option<&str>,
    cfgs: Option<&[Cfg]>,
) -> CargoResult<Graph<'a>> {
    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };
    let node = Node {
        id: root,
        pack: packages.get(root)?
    };
    graph.nodes.insert(root, graph.graph.add_node(node));

    let mut pending = vec![root];

    while let Some(pkg_id) = pending.pop() {
        let idx = graph.nodes[&pkg_id];
        let pkg = packages.get(pkg_id)?;

        for raw_dep_id in resolve.deps_not_replaced(pkg_id) {
            let it = pkg.dependencies()
                .iter()
                .filter(|d| d.matches_id(raw_dep_id))
                .filter(|d| {
                    d.platform()
                        .and_then(|p| target.map(|t| p.matches(t, cfgs)))
                        .unwrap_or(true)
                });
            let dep_id = match resolve.replacement(raw_dep_id) {
                Some(id) => id,
                None => raw_dep_id,
            };
            for dep in it {
                let dep_idx = match graph.nodes.entry(dep_id) {
                    Entry::Occupied(e) => *e.get(),
                    Entry::Vacant(e) => {
                        pending.push(dep_id);
                        let node = Node {
                            id: dep_id,
                            pack: packages.get(dep_id)?
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

fn print_tree<'a>(
    package: &'a PackageId,
    graph: &Graph<'a>,
    format: &Pattern,
    direction: EdgeDirection,
    symbols: &Symbols,
    prefix: Prefix,
    all: bool,
) {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];

    let node = &graph.graph[graph.nodes[&package]];
    print_dependency(
        node,
        &graph,
        format,
        direction,
        symbols,
        &mut visited_deps,
        &mut levels_continue,
        prefix,
        all,
    );
}

/// Review please: Is this a sane way to do it for cargo-osha or should the
/// output be a table perhaps? Or a simple list?
fn print_dependency<'a>(
    package: &Node<'a>,
    graph: &Graph<'a>,
    format: &Pattern,
    direction: EdgeDirection,
    symbols: &Symbols,
    visited_deps: &mut HashSet<&'a PackageId>,
    levels_continue: &mut Vec<bool>,
    prefix: Prefix,
    all: bool,
) {
    let new = all || visited_deps.insert(package.id);
    //let star = if new { "" } else { " (*)" };

    match prefix {
        Prefix::Depth => print!("{} ", levels_continue.len()),
        Prefix::Indent => {
            if let Some((&last_continues, rest)) = levels_continue.split_last() {
                for &continues in rest {
                    let c = if continues { symbols.down } else { " " };
                    print!("{}   ", c);
                }

                let c = if last_continues {
                    symbols.tee
                } else {
                    symbols.ell
                };
                print!("{0}{1}{1} ", c, symbols.right);
            }
        },
        Prefix::None => ()
    }
    
    // TODO: Add command line flag for this.
    let allow_partial_results = false;

    let counters = find_unsafe(
        package.pack.root(),
        allow_partial_results);
    let counts = [
        counters.functions.unsafe_num,
        counters.exprs.unsafe_num,
        counters.itemimpls.unsafe_num,
        counters.itemtraits.unsafe_num,
        counters.methods.unsafe_num
    ];
    let unsafe_found = counts.iter().any(|c| *c > 0);
    let rad = if unsafe_found { "☢" } else { "" };
    let compact_unsafe_info = 
        counts
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<String>>()
        .join(", ");
    let line = format!(
        "{} ({})",
        format.display(
            package.id,
            package.pack.manifest().metadata()),
        compact_unsafe_info);
    let line = if unsafe_found { line.red().bold() } else { line.green() };
    println!("{} {}", line, rad);
    if !new {
        return;
    }

    let mut normal = vec![];
    let mut build = vec![];
    let mut development = vec![];
    for edge in graph
        .graph
        .edges_directed(graph.nodes[&package.id], direction)
    {
        let dep = match direction {
            EdgeDirection::Incoming => &graph.graph[edge.source()],
            EdgeDirection::Outgoing => &graph.graph[edge.target()],
        };
        match *edge.weight() {
            Kind::Normal => normal.push(dep),
            Kind::Build => build.push(dep),
            Kind::Development => development.push(dep),
        }
    }

    print_dependency_kind(
        Kind::Normal,
        normal,
        graph,
        format,
        direction,
        symbols,
        visited_deps,
        levels_continue,
        prefix,
        all,
    );
    print_dependency_kind(
        Kind::Build,
        build,
        graph,
        format,
        direction,
        symbols,
        visited_deps,
        levels_continue,
        prefix,
        all,
    );
    print_dependency_kind(
        Kind::Development,
        development,
        graph,
        format,
        direction,
        symbols,
        visited_deps,
        levels_continue,
        prefix,
        all,
    );
}

fn print_dependency_kind<'a>(
    kind: Kind,
    mut deps: Vec<&Node<'a>>,
    graph: &Graph<'a>,
    format: &Pattern,
    direction: EdgeDirection,
    symbols: &Symbols,
    visited_deps: &mut HashSet<&'a PackageId>,
    levels_continue: &mut Vec<bool>,
    prefix: Prefix,
    all: bool,
) {
    if deps.is_empty() {
        return;
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    deps.sort_by_key(|n| n.id);

    let name = match kind {
        Kind::Normal => None,
        Kind::Build => Some("[build-dependencies]"),
        Kind::Development => Some("[dev-dependencies]"),
    };
    if let Prefix::Indent = prefix {
        if let Some(name) = name {
            for &continues in &**levels_continue {
                let c = if continues { symbols.down } else { " " };
                print!("{}   ", c);
            }

            println!("{}", name);
        }
    }

    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        print_dependency(
            dependency,
            graph,
            format,
            direction,
            symbols,
            visited_deps,
            levels_continue,
            prefix,
            all,
        );
        levels_continue.pop();
    }
}
