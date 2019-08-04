//! This module provides the bulk of the code for the `cargo-geiger` executable.

// TODO: Review the module structure in this crate. There is very tight coupling
// between the main.rs and this module. Should this module be split into smaller
// parts? The printing and scanning can probably be further decoupled to provide
// a better base for adding more output formats.

// TODO: Investigate how cargo-clippy is implemented. Is it using syn?  Is is
// using rustc? Is it implementing a compiler plugin?

extern crate cargo;
extern crate colored;
extern crate env_logger;
extern crate failure;
extern crate geiger;
extern crate petgraph;
extern crate structopt;
extern crate walkdir;

use self::walkdir::DirEntry;
use self::walkdir::WalkDir;
use crate::format::Pattern;
use cargo::core::compiler::CompileMode;
use cargo::core::compiler::Executor;
use cargo::core::compiler::Unit;
use cargo::core::dependency::Kind;
use cargo::core::manifest::TargetKind;
use cargo::core::package::PackageSet;
use cargo::core::registry::PackageRegistry;
use cargo::core::resolver::Method;
use cargo::core::shell::Verbosity;
use cargo::core::Target;
use cargo::core::{Package, PackageId, Resolve, Workspace};
use cargo::ops;
use cargo::ops::CleanOptions;
use cargo::ops::CompileOptions;
use cargo::util::paths;
use cargo::util::ProcessBuilder;
use cargo::util::{self, important_paths, CargoResult, Cfg};
use cargo::Config;
use colored::Colorize;
use geiger::find_rs_files_in_dir;
use geiger::find_unsafe_in_file;
use geiger::Count;
use geiger::CounterBlock;
use geiger::IncludeTests;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::EdgeDirection;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::str::{self, FromStr};
use std::sync::Arc;
use std::sync::Mutex;

#[cfg(not(target_os = "windows"))]
pub const LOCK: &str = "🔒";

#[cfg(not(target_os = "windows"))]
pub const QUESTION_MARK: &str = "❓";

#[cfg(not(target_os = "windows"))]
pub const RADS: &str = "☢️";

#[derive(Debug)]
pub enum RsResolveError {
    Walkdir(walkdir::Error),

    /// Like io::Error but with the related path.
    Io(io::Error, PathBuf),

    /// Would like cargo::Error here, but it's private, why?
    /// This is still way better than a panic though.
    Cargo(String),

    /// This should not happen unless incorrect assumptions have been made in
    /// cargo-geiger about how the cargo API works.
    ArcUnwrap(),

    /// Failed to get the inner context out of the mutex.
    InnerContextMutex(String),

    /// Failed to parse a .dep file.
    DepParse(String, PathBuf),
}

impl Error for RsResolveError {}

/// Forward Display to Debug.
impl fmt::Display for RsResolveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl From<PoisonError<CustomExecutorInnerContext>> for RsResolveError {
    fn from(e: PoisonError<CustomExecutorInnerContext>) -> Self {
        RsResolveError::InnerContextMutex(e.to_string())
    }
}

#[derive(Debug, Default)]
pub struct PackageMetricsRoot {
    pub used: PackageMetrics,
    pub not_used: PackageMetrics,
}

#[derive(Debug, Default)]
pub struct PackageMetrics {
    pub counters: CounterBlock,
    pub entry_points: EntryPointMetrics,
}

#[derive(Debug, Default)]
pub struct EntryPointMetrics {
    /// The number of build target entry point `.rs` files that declare
    /// `!#[forbid(unsafe_code)]`.
    pub forbids_unsafe: u64,

    /// The number of build target entry point `.rs` files that does __not__
    /// declare `!#[forbid(unsafe_code)]`.
    pub allows_unsafe: u64,
}

/// TODO: Write documentation.
pub struct GeigerContext {
    pub pack_id_to_metrics: HashMap<PackageId, PackageMetricsRoot>,
    pub rs_files_used: HashMap<PathBuf, u32>,
}

// TODO: Review this. The same code exist in the `geiger` library crate, but is
// private since I don't want to expose `WalkDir` in the public API for this
// simple function. Is this function available in WalkDir already or something
// similar? If not, open a github issue and ask if this would be appropriate as
// a PR. Don't use to_string_lossy and return a result or option instead.
pub fn is_file_with_ext(entry: &DirEntry, file_ext: &str) -> bool {
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

// TODO: Make a wrapper type for canonical paths and hide all mutable access.

/// Provides information needed to scan for crate root
/// `#![forbid(unsafe_code)]`.
/// The wrapped PathBufs are canonicalized.
enum RsFile {
    /// Library entry point source file, usually src/lib.rs
    LibRoot(PathBuf),

    /// Executable entry point source file, usually src/main.rs
    BinRoot(PathBuf),

    /// Not sure if this is relevant but let's be conservative for now.
    CustomBuildRoot(PathBuf),

    /// All other .rs files.
    Other(PathBuf),
}

fn find_rs_files_in_package(pack: &Package) -> Vec<RsFile> {
    // Find all build target entry point source files.
    let mut canon_targets = HashMap::new();
    for t in pack.targets() {
        let path = t.src_path().path();
        let path = match path {
            None => continue,
            Some(p) => p,
        };
        if !path.exists() {
            // A package published to crates.io is not required to include
            // everything. We have to skip this build target.
            continue;
        }
        let canon = path
            .canonicalize() // will Err on non-existing paths.
            .expect("canonicalize for build target path failed."); // FIXME
        let targets = canon_targets.entry(canon).or_insert_with(Vec::new);
        targets.push(t);
    }
    let mut out = Vec::new();
    for p in find_rs_files_in_dir(pack.root()) {
        if !canon_targets.contains_key(&p) {
            out.push(RsFile::Other(p));
        }
    }
    for (k, v) in canon_targets.into_iter() {
        for target in v {
            out.push(into_rs_code_file(target.kind(), k.clone()));
        }
    }
    out
}

fn into_rs_code_file(kind: &TargetKind, path: PathBuf) -> RsFile {
    match kind {
        TargetKind::Lib(_) => RsFile::LibRoot(path),
        TargetKind::Bin => RsFile::BinRoot(path),
        TargetKind::Test => RsFile::Other(path),
        TargetKind::Bench => RsFile::Other(path),
        TargetKind::ExampleLib(_) => RsFile::Other(path),
        TargetKind::ExampleBin => RsFile::Other(path),
        TargetKind::CustomBuild => RsFile::CustomBuildRoot(path),
    }
}

fn find_rs_files_in_packages<'a>(
    packs: &'a [&Package],
) -> impl Iterator<Item = (PackageId, RsFile)> + 'a {
    packs.iter().flat_map(|pack| {
        find_rs_files_in_package(pack)
            .into_iter()
            .map(move |path| (pack.package_id(), path))
    })
}

pub fn find_unsafe_in_packages<'a, 'b>(
    packs: &'a PackageSet<'b>,
    mut rs_files_used: HashMap<PathBuf, u32>,
    allow_partial_results: bool,
    include_tests: IncludeTests,
    verbosity: Verbosity,
) -> GeigerContext {
    let mut pack_id_to_metrics = HashMap::new();
    let packs = packs.get_many(packs.package_ids()).unwrap();
    let pack_code_files = find_rs_files_in_packages(&packs);
    for (pack_id, rs_code_file) in pack_code_files {
        let (is_entry_point, p) = match rs_code_file {
            RsFile::LibRoot(pb) => (true, pb),
            RsFile::BinRoot(pb) => (true, pb),
            RsFile::CustomBuildRoot(pb) => (true, pb),
            RsFile::Other(pb) => (false, pb),
        };
        let p = &p;
        let scan_counter = rs_files_used.get_mut(p);
        let used_by_build = match scan_counter {
            Some(c) => {
                // TODO: Add proper logging.
                if verbosity == Verbosity::Verbose {
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
                if verbosity == Verbosity::Verbose {
                    println!("Not used in build: {}", p.display());
                }
                false
            }
        };
        match find_unsafe_in_file(p, include_tests) {
            Err(e) => {
                if allow_partial_results {
                    eprintln!(
                        "Failed to parse file: {}, {:?} ",
                        p.display(),
                        e
                    );
                } else {
                    panic!("Failed to parse file: {}, {:?} ", p.display(), e);
                }
            }
            Ok(file_metrics) => {
                let pack_metrics_root = pack_id_to_metrics
                    .entry(pack_id)
                    .or_insert_with(PackageMetricsRoot::default);
                let target = if used_by_build {
                    &mut pack_metrics_root.used
                } else {
                    &mut pack_metrics_root.not_used
                };
                target.counters =
                    target.counters.clone() + file_metrics.counters;
                if is_entry_point {
                    let ep = &mut target.entry_points;
                    if file_metrics.forbids_unsafe {
                        ep.forbids_unsafe += 1;
                    } else {
                        ep.allows_unsafe += 1;
                    }
                }
            }
        }
    }
    GeigerContext {
        pack_id_to_metrics,
        rs_files_used,
    }
}

pub enum Charset {
    Utf8,
    Ascii,
}

#[derive(Clone, Copy)]
pub enum Prefix {
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

pub struct Symbols {
    down: &'static str,
    tee: &'static str,
    ell: &'static str,
    right: &'static str,
}

pub const UTF8_SYMBOLS: Symbols = Symbols {
    down: "│",
    tee: "├",
    ell: "└",
    right: "─",
};

pub const ASCII_SYMBOLS: Symbols = Symbols {
    down: "|",
    tee: "|",
    ell: "`",
    right: "-",
};

pub struct PrintConfig<'a> {
    /// Don't truncate dependencies that have already been displayed.
    pub all: bool,

    pub verbosity: Verbosity,
    pub direction: EdgeDirection,
    pub prefix: Prefix,

    // Is anyone using this? This is a carry-over from cargo-tree.
    // TODO: Open a github issue to discuss deprecation.
    pub format: &'a Pattern,

    pub symbols: &'a Symbols,
    pub allow_partial_results: bool,
    pub include_tests: IncludeTests,
}

/// Trigger a `cargo clean` + `cargo check` and listen to the cargo/rustc
/// communication to figure out which source files were used by the build.
pub fn resolve_rs_file_deps(
    copt: &CompileOptions,
    ws: &Workspace,
) -> Result<HashMap<PathBuf, u32>, RsResolveError> {
    let config = ws.config();
    // Need to run a cargo clean to identify all new .d deps files.
    // TODO: Figure out how this can be avoided to improve performance, clean
    // Rust builds are __slow__.
    let clean_opt = CleanOptions {
        config: &config,
        spec: vec![],
        target: None,
        release: false,
        doc: false,
    };
    ops::clean(ws, &clean_opt)
        .map_err(|e| RsResolveError::Cargo(e.to_string()))?;
    let inner_arc = Arc::new(Mutex::new(CustomExecutorInnerContext::default()));
    {
        let cust_exec = CustomExecutor {
            cwd: config.cwd().to_path_buf(),
            inner_ctx: inner_arc.clone(),
        };
        let exec: Arc<dyn Executor> = Arc::new(cust_exec);
        ops::compile_with_exec(ws, &copt, &exec)
            .map_err(|e| RsResolveError::Cargo(e.to_string()))?;
    }
    let ws_root = ws.root().to_path_buf();
    let inner_mutex =
        Arc::try_unwrap(inner_arc).map_err(|_| RsResolveError::ArcUnwrap())?;
    let (rs_files, out_dir_args) = {
        let ctx = inner_mutex.into_inner()?;
        (ctx.rs_file_args, ctx.out_dir_args)
    };
    let mut hm = HashMap::<PathBuf, u32>::new();
    for out_dir in out_dir_args {
        // TODO: Figure out if the `.d` dep files are used by one or more rustc
        // calls. It could be useful to know which `.d` dep files belong to
        // which rustc call. That would allow associating each `.rs` file found
        // in each dep file with a PackageId.
        for ent in WalkDir::new(&out_dir) {
            let ent = ent.map_err(RsResolveError::Walkdir)?;
            if !is_file_with_ext(&ent, "d") {
                continue;
            }
            let deps = parse_rustc_dep_info(ent.path()).map_err(|e| {
                RsResolveError::DepParse(
                    e.to_string(),
                    ent.path().to_path_buf(),
                )
            })?;
            let canon_paths = deps
                .into_iter()
                .flat_map(|t| t.1)
                .map(PathBuf::from)
                .map(|pb| ws_root.join(pb))
                .map(|pb| {
                    pb.canonicalize().map_err(|e| RsResolveError::Io(e, pb))
                });
            for p in canon_paths {
                hm.insert(p?, 0);
            }
        }
    }
    for pb in rs_files {
        // rs_files must already be canonicalized
        hm.insert(pb, 0);
    }
    Ok(hm)
}

/// Copy-pasted (almost) from the private module cargo::core::compiler::fingerprint.
///
/// TODO: Make a PR to the cargo project to expose this function or to expose
/// the dependency data in some other way.
fn parse_rustc_dep_info(
    rustc_dep_info: &Path,
) -> CargoResult<Vec<(String, Vec<String>)>> {
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
                    file.push_str(
                        deps.next()
                            .expect("malformed dep-info format, trailing \\"),
                    );
                }
                ret.push(file);
            }
            Ok((target.to_string(), ret))
        })
        .collect()
}

#[derive(Debug, Default)]
struct CustomExecutorInnerContext {
    /// Stores all lib.rs, main.rs etc. passed to rustc during the build.
    rs_file_args: HashSet<PathBuf>,

    /// Investigate if this needs to be intercepted like this or if it can be
    /// looked up in a nicer way.
    out_dir_args: HashSet<PathBuf>,
}

use std::sync::PoisonError;

/// A cargo Executor to intercept all build tasks and store all ".rs" file
/// paths for later scanning.
///
/// TODO: This is the place(?) to make rustc perform macro expansion to allow
/// scanning of the the expanded code. (incl. code generated by build.rs).
/// Seems to require nightly rust.
#[derive(Debug)]
struct CustomExecutor {
    /// Current work dir
    cwd: PathBuf,

    /// Needed since multiple rustc calls can be in flight at the same time.
    inner_ctx: Arc<Mutex<CustomExecutorInnerContext>>,
}

use std::error::Error;
use std::fmt;

#[derive(Debug)]
enum CustomExecutorError {
    OutDirKeyMissing(String),
    OutDirValueMissing(String),
    InnerContextMutex(String),
    Io(io::Error, PathBuf),
}

impl Error for CustomExecutorError {}

/// Forward Display to Debug. See the crate root documentation.
impl fmt::Display for CustomExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl Executor for CustomExecutor {
    /// In case of an `Err`, Cargo will not continue with the build process for
    /// this package.
    fn exec(
        &self,
        cmd: ProcessBuilder,
        _id: PackageId,
        _target: &Target,
        _mode: CompileMode,
    ) -> CargoResult<()> {
        let args = cmd.get_args();
        let out_dir_key = OsString::from("--out-dir");
        let out_dir_key_idx =
            args.iter().position(|s| *s == out_dir_key).ok_or_else(|| {
                CustomExecutorError::OutDirKeyMissing(cmd.to_string())
            })?;
        let out_dir = args
            .get(out_dir_key_idx + 1)
            .ok_or_else(|| {
                CustomExecutorError::OutDirValueMissing(cmd.to_string())
            })
            .map(PathBuf::from)?;

        // This can be different from the cwd used to launch the wrapping cargo
        // plugin. Discovered while fixing
        // https://github.com/anderejd/cargo-geiger/issues/19
        let cwd = cmd
            .get_cwd()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.cwd.to_owned());

        {
            // Scope to drop and release the mutex before calling rustc.
            let mut ctx = self.inner_ctx.lock().map_err(|e| {
                CustomExecutorError::InnerContextMutex(e.to_string())
            })?;
            for tuple in args
                .iter()
                .map(|s| (s, s.to_string_lossy().to_lowercase()))
                .filter(|t| t.1.ends_with(".rs"))
            {
                let raw_path = cwd.join(tuple.0);
                let p = raw_path
                    .canonicalize()
                    .map_err(|e| CustomExecutorError::Io(e, raw_path))?;
                ctx.rs_file_args.insert(p);
            }
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
        _id: PackageId,
        _target: &Target,
        _mode: CompileMode,
        _handle_stdout: &mut dyn FnMut(&str) -> CargoResult<()>,
        _handle_stderr: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
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
pub fn get_cfgs(
    config: &Config,
    target: &Option<String>,
    ws: &Workspace,
) -> CargoResult<Option<Vec<Cfg>>> {
    let mut process = util::process(&config.load_global_rustc(Some(ws))?.path);
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

pub fn workspace(
    config: &Config,
    manifest_path: Option<PathBuf>,
) -> CargoResult<Workspace> {
    let root = match manifest_path {
        Some(path) => path,
        None => important_paths::find_root_manifest_for_wd(config.cwd())?,
    };
    Workspace::new(&root, config)
}

pub fn registry<'a>(
    config: &'a Config,
    package: &Package,
) -> CargoResult<PackageRegistry<'a>> {
    let mut registry = PackageRegistry::new(config)?;
    registry.add_sources(Some(package.package_id().source_id()))?;
    Ok(registry)
}

pub fn resolve<'a, 'cfg>(
    registry: &mut PackageRegistry<'cfg>,
    ws: &'a Workspace<'cfg>,
    features: Option<String>,
    all_features: bool,
    no_default_features: bool,
) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let features =
        Method::split_features(&features.into_iter().collect::<Vec<_>>());
    let (packages, resolve) = ops::resolve_ws(ws)?;
    let method = Method::Required {
        dev_deps: true,
        features: &features,
        all_features,
        uses_default_features: !no_default_features,
    };
    let resolve = ops::resolve_with_previous(
        registry,
        ws,
        method,
        Some(&resolve),
        None,
        &[],
        true,
        true,
    )?;
    Ok((packages, resolve))
}

pub struct Node<'a> {
    id: PackageId,
    pack: &'a Package,
}

pub struct Graph<'a> {
    graph: petgraph::Graph<Node<'a>, Kind>,
    nodes: HashMap<PackageId, NodeIndex>,
}

/// Almost unmodified compared to the original in cargo-tree, should be fairly
/// simple to move this and the dependency graph structure out to a library.
/// TODO: Move this to a module to begin with.
pub fn build_graph<'a>(
    resolve: &'a Resolve,
    packages: &'a PackageSet,
    root: PackageId,
    target: Option<&str>,
    cfgs: Option<&[Cfg]>,
    extra_deps: ExtraDeps,
) -> CargoResult<Graph<'a>> {
    let mut graph = Graph {
        graph: petgraph::Graph::new(),
        nodes: HashMap::new(),
    };
    let node = Node {
        id: root,
        pack: packages.get_one(root)?,
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
                .filter(|d| d.matches_id(raw_dep_id))
                .filter(|d| extra_deps.allows(d.kind()))
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
                            pack: packages.get_one(dep_id)?,
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

pub fn print_tree<'a>(
    root_pack_id: PackageId,
    graph: &Graph<'a>,
    geiger_ctx: &GeigerContext,
    pc: &PrintConfig,
) {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];
    let node = &graph.graph[graph.nodes[&root_pack_id]];
    print_dependency(
        node,
        &graph,
        &mut visited_deps,
        &mut levels_continue,
        geiger_ctx,
        pc,
    );
}

enum DetectionStatus {
    NoneDetectedForbidsUnsafe,
    NoneDetectedAllowsUnsafe,
    UnsafeDetected,
}

fn print_dependency<'a>(
    package: &Node<'a>,
    graph: &Graph<'a>,
    visited_deps: &mut HashSet<PackageId>,
    levels_continue: &mut Vec<bool>,
    geiger_ctx: &GeigerContext,
    pc: &PrintConfig,
) {
    let new = pc.all || visited_deps.insert(package.id);
    let treevines = match pc.prefix {
        Prefix::Depth => format!("{} ", levels_continue.len()),
        Prefix::Indent => {
            let mut buf = String::new();
            if let Some((&last_continues, rest)) = levels_continue.split_last()
            {
                for &continues in rest {
                    let c = if continues { pc.symbols.down } else { " " };
                    buf.push_str(&format!("{}   ", c));
                }
                let c = if last_continues {
                    pc.symbols.tee
                } else {
                    pc.symbols.ell
                };
                buf.push_str(&format!("{0}{1}{1} ", c, pc.symbols.right));
            }
            buf
        }
        Prefix::None => "".into(),
    };

    // TODO: Try to be panic free and use Result everywhere, but separate tree
    // printing and metrics printing first. Use a callback or produce tree rows
    // through an Iterator together with the PackageId and map together the
    // complete row for printing in the caller code.
    let pack_metrics_root = geiger_ctx
        .pack_id_to_metrics
        .get(&package.id)
        .unwrap_or_else(|| {
            panic!("Failed to get unsafe counters for package: {}", package.id)
        });
    let unsafe_found = pack_metrics_root.used.counters.has_unsafe();
    let all_used_targets_forbids_unsafe =
        pack_metrics_root.used.entry_points.forbids_unsafe >= 1
            && pack_metrics_root.used.entry_points.allows_unsafe == 0;

    let detection_status = match (unsafe_found, all_used_targets_forbids_unsafe)
    {
        (false, true) => DetectionStatus::NoneDetectedForbidsUnsafe,
        (false, false) => DetectionStatus::NoneDetectedAllowsUnsafe,
        (true, _) => DetectionStatus::UnsafeDetected,
    };

    let colorize = |s: String| match detection_status {
        DetectionStatus::NoneDetectedForbidsUnsafe => s.green(),
        DetectionStatus::NoneDetectedAllowsUnsafe => s.normal(),
        DetectionStatus::UnsafeDetected => s.red().bold(),
    };

    // This is a hack, maybe some third party terminal emulators on windows does
    // support emoji? Are there reliable ways to detect this feature or is the
    // best case to lookup terminal emulator name and version? Some googling
    // suggests that recent Linux desktop environments do support colored emoji
    // in the terminal, so let's only disable emoji on Windows. Tested Pop_OS
    // 18.10, seems to print emoji in the default terminal just fine.
    #[cfg(not(target_os = "windows"))]
    let icon = match detection_status {
        DetectionStatus::NoneDetectedForbidsUnsafe => LOCK,
        DetectionStatus::NoneDetectedAllowsUnsafe => QUESTION_MARK,
        DetectionStatus::UnsafeDetected => RADS,
    };
    #[cfg(target_os = "windows")]
    let icon = match detection_status {
        DetectionStatus::NoneDetectedForbidsUnsafe => ":)".green(),
        DetectionStatus::NoneDetectedAllowsUnsafe => "? ".normal(),
        DetectionStatus::UnsafeDetected => "! ".red().bold(),
    };

    let dep_name = colorize(format!(
        "{}",
        pc.format
            .display(&package.id, package.pack.manifest().metadata())
    ));

    let unsafe_info = colorize(table_row(&pack_metrics_root));

    // Here comes some special control characters to position the cursor
    // properly for printing the last column containing the tree vines, after
    // the emoji icon. This is a workaround for a potential bug where the
    // radiation emoji will visually cover two characters in width but only
    // count as a single character if using the column formatting provided by
    // Rust. This could be unrelated to Rust and a quirk of this particular
    // symbol or something in the Terminal app on macOS.
    print!("{}  {}", unsafe_info, icon);
    print!("\r"); // Return the cursor to the start of the line.
    print!("\x1B[51C"); // Move the cursor 51 characters to the right.
    println!("{}{}", treevines, dep_name);

    if !new {
        return;
    }
    let mut normal = vec![];
    let mut build = vec![];
    let mut development = vec![];
    for edge in graph
        .graph
        .edges_directed(graph.nodes[&package.id], pc.direction)
    {
        let dep = match pc.direction {
            EdgeDirection::Incoming => &graph.graph[edge.source()],
            EdgeDirection::Outgoing => &graph.graph[edge.target()],
        };
        match *edge.weight() {
            Kind::Normal => normal.push(dep),
            Kind::Build => build.push(dep),
            Kind::Development => development.push(dep),
        }
    }
    let mut kinds = [
        (Kind::Normal, normal),
        (Kind::Build, build),
        (Kind::Development, development),
    ];
    for (kind, kind_deps) in kinds.iter_mut() {
        print_dependency_kind(
            *kind,
            kind_deps,
            graph,
            visited_deps,
            levels_continue,
            geiger_ctx,
            pc,
        );
    }
}

fn print_dependency_kind<'a>(
    kind: Kind,
    deps: &mut Vec<&Node<'a>>,
    graph: &Graph<'a>,
    visited_deps: &mut HashSet<PackageId>,
    levels_continue: &mut Vec<bool>,
    geiger_ctx: &GeigerContext,
    pc: &PrintConfig,
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
    if let Prefix::Indent = pc.prefix {
        if let Some(name) = name {
            print!("{}", table_row_empty());
            for &continues in &**levels_continue {
                let c = if continues { pc.symbols.down } else { " " };
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
            visited_deps,
            levels_continue,
            geiger_ctx,
            pc,
        );
        levels_continue.pop();
    }
}

// TODO: use a table library, or factor the tableness out in a smarter way
pub const UNSAFE_COUNTERS_HEADER: [&str; 6] = [
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

fn table_row(r: &PackageMetricsRoot) -> String {
    let fmt = |used: &Count, not_used: &Count| {
        format!("{}/{}", used.unsafe_, used.unsafe_ + not_used.unsafe_)
    };
    format!(
        "{: <10} {: <12} {: <6} {: <7} {: <7}",
        fmt(&r.used.counters.functions, &r.not_used.counters.functions),
        fmt(&r.used.counters.exprs, &r.not_used.counters.exprs),
        fmt(&r.used.counters.item_impls, &r.not_used.counters.item_impls),
        fmt(
            &r.used.counters.item_traits,
            &r.not_used.counters.item_traits
        ),
        fmt(&r.used.counters.methods, &r.not_used.counters.methods),
    )
}

pub enum ExtraDeps {
    All,
    Build,
    Dev,
    NoMore,
}

impl ExtraDeps {
    fn allows(&self, dep: Kind) -> bool {
        match (self, dep) {
            (_, Kind::Normal) => true,
            (ExtraDeps::All, _) => true,
            (ExtraDeps::Build, Kind::Build) => true,
            (ExtraDeps::Dev, Kind::Development) => true,
            _ => false,
        }
    }
}
