#![forbid(unsafe_code)]

#[macro_use]
extern crate structopt;
extern crate cargo;
extern crate colored;
extern crate env_logger;
extern crate failure;
extern crate petgraph;
extern crate syn;
extern crate walkdir;

use self::walkdir::DirEntry;
use self::walkdir::WalkDir;
use cargo::core::compiler::CompileMode;
use cargo::core::compiler::Executor;
use cargo::core::compiler::Unit;
use cargo::core::dependency::Kind;
use cargo::core::package::PackageSet;
use cargo::core::registry::PackageRegistry;
use cargo::core::resolver::Method;
use cargo::core::shell::Shell;
use cargo::core::Target;
use cargo::core::{Package, PackageId, Resolve, Workspace};
use cargo::ops;
use cargo::ops::CleanOptions;
use cargo::ops::CompileOptions;
use cargo::util::paths;
use cargo::util::ProcessBuilder;
use cargo::util::{self, important_paths, CargoResult, Cfg};
use cargo::{CliResult, Config};
use colored::*;
use format::Pattern;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::EdgeDirection;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::str::{self, FromStr};
use std::sync::Arc;
use std::sync::Mutex;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use syn::{visit, Expr, ImplItemMethod, ItemFn, ItemImpl, ItemTrait};

mod format;

#[derive(Debug, Copy, Clone, Default)]
pub struct Count {
    /// Number of safe items, in .rs files not used by the build.
    pub safe_unused: u64,

    /// Number of safe items, in .rs files used by the build.
    pub safe_used: u64,

    /// Number of unsafe items, in .rs files not used by the build.
    pub unsafe_unused: u64,

    /// Number of unsafe items, in .rs files used by the build.
    pub unsafe_used: u64,
}

impl Count {
    fn count(&mut self, is_unsafe: bool, used_by_build: bool) {
        match (is_unsafe, used_by_build) {
            (false, false) => self.safe_unused += 1,
            (false, true) => self.safe_used += 1,
            (true, false) => self.unsafe_unused += 1,
            (true, true) => self.unsafe_used += 1,
        }
    }
}

/// Unsafe usage metrics collection.
#[derive(Debug, Copy, Clone, Default)]
struct CounterBlock {
    pub functions: Count,
    pub exprs: Count,
    pub itemimpls: Count,
    pub itemtraits: Count,
    pub methods: Count,
}

impl CounterBlock {
    fn has_unsafe(&self) -> bool {
        self.functions.unsafe_used > 0
            || self.exprs.unsafe_used > 0
            || self.itemimpls.unsafe_used > 0
            || self.itemtraits.unsafe_used > 0
            || self.methods.unsafe_used > 0
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct GeigerSynVisitor {
    /// Unsafe usage metrics collected from .rs files not used by the build.
    pub counters: CounterBlock,

    /// Used by the Visit trait implementation to separate the metrics into
    /// "used by build" and "not used by build" based on if the .rs file was used
    /// in the build or not.
    pub used_by_build: bool,

    /// Used by the Visit trait implementation to track the traversal state.
    in_unsafe_block: bool,
}

impl<'ast> visit::Visit<'ast> for GeigerSynVisitor {
    fn visit_item_fn(&mut self, i: &ItemFn) {
        // fn definitions
        self.counters
            .functions
            .count(i.unsafety.is_some(), self.used_by_build);
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
                self.counters
                    .exprs
                    .count(self.in_unsafe_block, self.used_by_build);
                visit::visit_expr(self, other);
            }
        }
    }

    fn visit_item_impl(&mut self, i: &ItemImpl) {
        // unsafe trait impl's
        self.counters
            .itemimpls
            .count(i.unsafety.is_some(), self.used_by_build);
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, i: &ItemTrait) {
        // Unsafe traits
        self.counters
            .itemtraits
            .count(i.unsafety.is_some(), self.used_by_build);
        visit::visit_item_trait(self, i);
    }

    fn visit_impl_item_method(&mut self, i: &ImplItemMethod) {
        self.counters
            .methods
            .count(i.sig.unsafety.is_some(), self.used_by_build);
        visit::visit_impl_item_method(self, i);
    }
}

fn is_file_with_ext(entry: &DirEntry, file_ext: &str) -> bool {
    if !entry.file_type().is_file() {
        return false;
    }
    let p = entry.path();
    let ext = match p.extension() {
        Some(e) => e,
        None => return false,
    };
    // to_string_lossy is ok since we only want to match against an ASCII
    // compatible extension and we do not keep the possibly lossy result
    // around.
    ext.to_string_lossy() == file_ext
}

fn find_unsafe(
    p: &Path,
    allow_partial_results: bool,
    rs_files_used: &mut HashMap<PathBuf, u32>,
    verbose: bool,
) -> CounterBlock {
    let vis = &mut GeigerSynVisitor::default();
    let walker = WalkDir::new(p).into_iter();
    for entry in walker {
        let entry = entry.expect("walkdir error, TODO: Implement error handling");
        if !is_file_with_ext(&entry, "rs") {
            continue;
        }
        let p = entry.path();
        let scan_counter = rs_files_used.get_mut(p);
        // TODO: fix GeigerSynVisitor to handle "included" and "excluded" modes.
        let in_build = match scan_counter {
            Some(c) => {
                // TODO: Add proper logging.
                if verbose {
                    println!("Used in build: {}", p.display());
                }
                // This .rs file path was found by intercepting rustc arguments
                // or by parsing the .d files produced by rustc. Here we
                // increase the counter for this path to mark that this file
                // has been scanned. Warnings will be printed for .rs files in
                // this collection with a count of 0 (has not been scanned). If
                // this happens, it could indicate a logic error or some
                // incorrect assumption in cargo-geiger.
                *c += 1;
                true
            }
            None => {
                // This file was not used in the build triggered by
                // cargo-geiger, but it should be scanned anyways to provide
                // both "in build" and "not in build" stats.
                // TODO: Add proper logging.
                if verbose {
                    println!("Not used in build: {}", p.display());
                }
                false
            }
        };
        let mut file = File::open(p).expect("Unable to open file");
        let mut src = String::new();
        file.read_to_string(&mut src).expect("Unable to read file");
        let syntax = match (allow_partial_results, syn::parse_file(&src)) {
            (_, Ok(s)) => s,
            (true, Err(e)) => {
                // TODO: Do proper error logging.
                println!("Failed to parse file: {}, {:?}", p.display(), e);
                continue;
            }
            (false, Err(e)) => panic!("Failed to parse file: {}, {:?} ", p.display(), e),
        };
        syn::visit::visit_file(vis, &syntax);
    }
    vis.counters
}

#[derive(StructOpt)]
#[structopt(bin_name = "cargo")]
enum Opts {
    #[structopt(
        name = "geiger",
        raw(
            setting = "AppSettings::UnifiedHelpMessage",
            setting = "AppSettings::DeriveDisplayOrder",
            setting = "AppSettings::DontCollapseArgsInUsage"
        )
    )]
    /// Display a tree visualization of a dependency graph
    Geiger(Args),
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

    #[structopt(
        long = "manifest-path",
        value_name = "PATH",
        parse(from_os_str)
    )]
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

    #[structopt(
        long = "charset",
        value_name = "CHARSET",
        default_value = "utf8"
    )]
    /// Character set to use in output: utf8, ascii
    charset: Charset,

    #[structopt(
        long = "format",
        short = "f",
        value_name = "FORMAT",
        default_value = "{p}"
    )]
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

    #[structopt(long = "compact")]
    /// Display compact output instead of table
    compact: bool,
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

    let Opts::Geiger(args) = Opts::from_args();

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
        &None, // TODO: add command line flag, new in cargo 0.27.
        &args.unstable_flags,
    )?;
    // TODO: Add a new default output format that adds all unsafe usage counts
    // to a single number?
    //     10 / 10     crate-one-0.1.0
    //     5  / 123    some-other-crate-0.1.0
    //     0  / 456    and-another-one-0.1.0
    let verbose = args.verbose != 0;
    let ws = workspace(config, args.manifest_path)?;
    let package = ws.current()?;
    let mut registry = registry(config, &package)?;
    let (packages, resolve) = resolve(
        &mut registry,
        &ws,
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

    let config_host = config.rustc(Some(&ws))?.host;
    let target = if args.all_targets {
        None
    } else {
        Some(args.target.as_ref().unwrap_or(&config_host).as_str())
    };

    let format = Pattern::new(&args.format).map_err(|e| failure::err_msg(e.to_string()))?;

    let cfgs = get_cfgs(config, &args.target, &ws)?;
    let graph = build_graph(
        &resolve,
        &packages,
        package.package_id(),
        target,
        cfgs.as_ref().map(|r| &**r),
    )?;

    let direction = if args.invert {
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

    let mut rs_files_used = HashMap::new();
    for path in resolve_rs_file_deps(&config, &ws) {
        rs_files_used.insert(path, 0);
    }

    if verbose {
        // Print all .rs files found through the .d files, in sorted order.
        let mut paths = rs_files_used
            .keys()
            .map(|k| k.to_owned())
            .collect::<Vec<PathBuf>>();
        paths.sort();
        paths
            .iter()
            .for_each(|p| println!("Used by build (sorted): {}", p.display()));
    }

    println!();
    if args.compact {
        println!(
            "{}",
            "Compact unsafe info: (functions, expressions, impls, traits, methods)".bold()
        );
    } else {
        println!(
            "{}",
            UNSAFE_COUNTERS_HEADER
                .iter()
                .map(|s| s.to_owned())
                .collect::<Vec<_>>()
                .join(" ")
                .bold()
        );
    }
    println!();
    print_tree(
        root,
        &graph,
        &format,
        direction,
        symbols,
        prefix,
        args.all,
        args.compact,
        &mut rs_files_used,
        verbose,
    );
    rs_files_used
        .iter()
        .filter(|(_k, v)| **v == 0)
        .for_each(|(k, _v)| {
            println!(
                "WARNING: Dependency file was never scanned: {}",
                k.display()
            )
        });
    Ok(())
}

/// TODO: Implement error handling and return Result.
fn resolve_rs_file_deps(config: &Config, ws: &Workspace) -> impl Iterator<Item = PathBuf> {
    // Need to run a cargo clean to identify all new .d deps files.
    let clean_opt = CleanOptions {
        config: &config,
        spec: vec![],
        target: None,
        release: false,
        doc: false,
    };
    ops::clean(ws, &clean_opt).unwrap();
    let copt = CompileOptions::new(&config, CompileMode::Check { test: false }).unwrap();
    let executor = Arc::new(CustomExecutor {
        ..Default::default()
    });
    ops::compile_with_exec(ws, &copt, executor.clone()).unwrap();
    let executor = Arc::try_unwrap(executor).unwrap();
    let (rs_files, out_dir_args) = {
        let inner = executor.into_inner();
        (inner.rs_file_args, inner.out_dir_args)
    };
    out_dir_args
        .into_iter()
        .flat_map(|dir| WalkDir::new(dir).into_iter())
        .map(|entry| entry.expect("walkdir error, TODO: Implement error handling"))
        .filter(|entry| is_file_with_ext(&entry, "d"))
        .flat_map(|entry| parse_rustc_dep_info(entry.path()).unwrap())
        .flat_map(|tuple| tuple.1)
        .map(|s| PathBuf::from(s))
        .map(|pb| pb.canonicalize().unwrap())
        .chain(rs_files)
}

/// Copy-pasted from the private module cargo::core::compiler::fingerprint.
/// TODO: Make a PR to the cargo project to expose this function or to expose
/// the dependency data in some other way.
pub fn parse_rustc_dep_info(rustc_dep_info: &Path) -> CargoResult<Vec<(String, Vec<String>)>> {
    let contents = paths::read(rustc_dep_info)?;
    contents
        .lines()
        .filter_map(|l| l.find(": ").map(|i| (l, i)))
        .map(|(line, pos)| {
            let target = &line[..pos];
            let mut deps = line[pos + 2..].split_whitespace();
            let mut ret = Vec::new();
            while let Some(s) = deps.next() {
                let mut file = s.to_string();
                while file.ends_with('\\') {
                    file.pop();
                    file.push(' ');
                    //file.push_str(deps.next().ok_or_else(|| {
                    //internal("malformed dep-info format, trailing \\".to_string())
                    //})?);
                    file.push_str(deps.next().expect("malformed dep-info format, trailing \\"));
                }
                ret.push(file);
            }
            Ok((target.to_string(), ret))
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct CustomExecutorInnerContext {
    /// Stores all lib.rs, main.rs etc. passed to rustc during the build.
    pub rs_file_args: HashSet<PathBuf>,

    /// Investigate if this needs to be intercepted like this or if it can be
    /// looked up in a nicer way.
    pub out_dir_args: HashSet<PathBuf>,
}

/// A cargo Executor to intercept all build tasks and store all ".rs" file
/// paths for later scanning.
///
/// TODO: This is the place to make rustc perform macro expansion to allow
/// scanning of the the expanded code. (incl. code generated by build.rs).
/// Seems to require nightly rust.
#[derive(Debug, Default)]
pub struct CustomExecutor {
    /// Needed since multiple rustc calls can be in flight at the same time.
    pub inner_ctx: Mutex<CustomExecutorInnerContext>,
}

impl CustomExecutor {
    pub fn into_inner(self) -> CustomExecutorInnerContext {
        self.inner_ctx.into_inner().unwrap()
    }
}

impl Executor for CustomExecutor {
    /// In case of an `Err`, Cargo will not continue with the build process for
    /// this package.
    fn exec(&self, cmd: ProcessBuilder, _id: &PackageId, _target: &Target) -> CargoResult<()> {
        // TODO: It seems like rustc must do its thing before we can get the
        // source file list for each unit. Find and read the ".d" files should
        // be used for that.
        let args = cmd.get_args();
        let out_dir_key = OsString::from("--out-dir");
        let out_dir_key_idx = match args.iter().position(|s| *s == out_dir_key) {
            Some(i) => i,
            None => panic!("Expected to find --out-dir in: {}", cmd),
        };
        let out_dir = match args.iter().nth(out_dir_key_idx + 1) {
            Some(s) => PathBuf::from(s),
            None => panic!("Expected a path after --out-dir in: {}", cmd),
        };
        {
            // Scope to drop and release the mutex before calling rustc.
            let mut ctx = self.inner_ctx.lock().unwrap();
            args.iter()
                .map(|s| (s, s.to_string_lossy().to_lowercase()))
                .filter(|t| t.1.ends_with(".rs"))
                .for_each(|t| {
                    ctx.rs_file_args
                        .insert(PathBuf::from(t.0).canonicalize().unwrap());
                });
            ctx.out_dir_args.insert(out_dir);
        }
        cmd.exec()?;
        Ok(())
    }

    /// TODO: Investigate if this returns the information we need through
    /// stdout or stderr.
    fn exec_json(
        &self,
        _cmd: ProcessBuilder,
        _id: &PackageId,
        _target: &Target,
        _handle_stdout: &mut FnMut(&str) -> CargoResult<()>,
        _handle_stderr: &mut FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
        //cmd.exec_with_streaming(handle_stdout, handle_stderr, false)?;
        //Ok(())
        unimplemented!();
    }

    /// Queried when queuing each unit of work. If it returns true, then the
    /// unit will always be rebuilt, independent of whether it needs to be.
    fn force_rebuild(&self, _unit: &Unit) -> bool {
        true // Overriding the default to force all units to be processed.
    }
}

/// TODO: Write proper documentation for this.
/// This function seems to be looking up the active flags for conditional
/// compilation (cargo::util::Cfg instances).
fn get_cfgs(
    config: &Config,
    target: &Option<String>,
    ws: &Workspace,
) -> CargoResult<Option<Vec<Cfg>>> {
    let mut process = util::process(&config.rustc(Some(ws))?.path);
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
    Ok(Some(
        lines.map(Cfg::from_str).collect::<CargoResult<Vec<_>>>()?,
    ))
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
    ws: &'a Workspace<'cfg>,
    features: Option<String>,
    all_features: bool,
    no_default_features: bool,
) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let features = Method::split_features(&features.into_iter().collect::<Vec<_>>());

    let (packages, resolve) = ops::resolve_ws(ws)?;

    let method = Method::Required {
        dev_deps: true,
        features: &features,
        all_features,
        uses_default_features: !no_default_features,
    };

    let resolve =
        ops::resolve_with_previous(registry, ws, method, Some(&resolve), None, &[], true, true)?;
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
/// TODO: Move this to a module to begin with.
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
        pack: packages.get(root)?,
    };
    graph.nodes.insert(root, graph.graph.add_node(node));

    let mut pending = vec![root];

    while let Some(pkg_id) = pending.pop() {
        let idx = graph.nodes[&pkg_id];
        let pkg = packages.get(pkg_id)?;

        for raw_dep_id in resolve.deps_not_replaced(pkg_id) {
            let it = pkg
                .dependencies()
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
                            pack: packages.get(dep_id)?,
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
    compact_output: bool,
    rs_files_used: &mut HashMap<PathBuf, u32>,
    verbose: bool,
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
        compact_output,
        rs_files_used,
        verbose,
    );
}

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
    compact_output: bool,
    rs_files_used: &mut HashMap<PathBuf, u32>,
    verbose: bool,
) {
    let new = all || visited_deps.insert(package.id);
    let treevines = match prefix {
        Prefix::Depth => format!("{} ", levels_continue.len()),
        Prefix::Indent => {
            let mut buf = String::new();
            if let Some((&last_continues, rest)) = levels_continue.split_last() {
                for &continues in rest {
                    let c = if continues { symbols.down } else { " " };
                    buf.push_str(&format!("{}   ", c));
                }
                let c = if last_continues {
                    symbols.tee
                } else {
                    symbols.ell
                };
                buf.push_str(&format!("{0}{1}{1} ", c, symbols.right));
            }
            buf
        }
        Prefix::None => "".into(),
    };

    // TODO: Add command line flag for this and make it default to false.
    let allow_partial_results = true;

    // TODO: Find and collect unsafe stats as a separate pass over the deps
    // tree before printing.
    let counters = find_unsafe(
        package.pack.root(),
        allow_partial_results,
        rs_files_used,
        verbose,
    );
    let unsafe_found = counters.has_unsafe();
    let colorize = |s: String| {
        if unsafe_found {
            s.red().bold()
        } else {
            s.green()
        }
    };
    let rad = if unsafe_found { "☢" } else { "" };
    let dep_name = colorize(format!(
        "{}",
        format.display(package.id, package.pack.manifest().metadata())
    ));
    let unsafe_info = colorize(table_row(&counters));
    println!("{}  {: <1} {}{}", unsafe_info, rad, treevines, dep_name);
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
        compact_output,
        rs_files_used,
        verbose,
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
        compact_output,
        rs_files_used,
        verbose,
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
        compact_output,
        rs_files_used,
        verbose,
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
    compact_output: bool,
    rs_files_used: &mut HashMap<PathBuf, u32>,
    verbose: bool,
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
            if !compact_output {
                print!("{}", table_row_empty());
            }
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
            compact_output,
            rs_files_used,
            verbose,
        );
        levels_continue.pop();
    }
}

// TODO: use a table library, or factor the tableness out in a smarter way
const UNSAFE_COUNTERS_HEADER: [&'static str; 6] = [
    "Functions ",
    "Expressions ",
    "Impls ",
    "Traits ",
    "Methods ",
    "Dependency",
];

fn table_row_empty() -> String {
    " ".repeat(
        UNSAFE_COUNTERS_HEADER
            .iter()
            .take(5)
            .map(|s| s.len())
            .sum::<usize>()
            + UNSAFE_COUNTERS_HEADER.len()
            + 1,
    )
}

fn table_row(cb: &CounterBlock) -> String {
    format!(
        "{: <9}  {: <11}  {: <5}  {: <6}  {: <7}",
        cb.functions.unsafe_used,
        cb.exprs.unsafe_used,
        cb.itemimpls.unsafe_used,
        cb.itemtraits.unsafe_used,
        cb.methods.unsafe_used,
    )
}
