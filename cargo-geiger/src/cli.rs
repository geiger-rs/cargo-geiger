//! This module provides the bulk of the code for the `cargo-geiger` executable.

// TODO: Review the module structure in this crate. There is very tight coupling
// between the main.rs and this module. Should this module be split into smaller
// parts? The printing and scanning can probably be further decoupled to provide
// a better base for adding more output formats.

// TODO: Investigate how cargo-clippy is implemented. Is it using syn?  Is is
// using rustc? Is it implementing a compiler plugin?

// TODO: Consider making this a lib.rs (again) and expose a full API, excluding
// only the terminal output..? That API would be dependent on cargo.

extern crate cargo;
extern crate colored;
extern crate console;
extern crate env_logger;
extern crate geiger;
extern crate petgraph;
extern crate structopt;
extern crate walkdir;

use self::walkdir::DirEntry;
use self::walkdir::WalkDir;
use crate::format::Pattern;
use crate::Args;
use cargo::core::compiler::CompileMode;
use cargo::core::compiler::Executor;
use cargo::core::compiler::Unit;
use cargo::core::dependency::Kind;
use cargo::core::manifest::TargetKind;
use cargo::core::package::PackageSet;
use cargo::core::registry::PackageRegistry;
use cargo::core::resolver::ResolveOpts;
use cargo::core::shell::Verbosity;
use cargo::core::Target;
use cargo::core::{Package, PackageId, PackageIdSpec, Resolve, Workspace};
use cargo::ops;
use cargo::ops::CleanOptions;
use cargo::ops::CompileOptions;
use cargo::util::paths;
use cargo::util::ProcessBuilder;
use cargo::util::{self, important_paths, CargoResult, Cfg};
use cargo::CliResult;
use cargo::Config;
use colored::Colorize;
use geiger::find_unsafe_in_file;
use geiger::Count;
use geiger::CounterBlock;
use geiger::IncludeTests;
use geiger::RsFileMetrics;
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

// ---------- BEGIN: Public items ----------

#[derive(Clone, Copy, PartialEq)]
pub enum Charset {
    Utf8,
    Ascii,
}

pub enum ExtraDeps {
    All,
    Build,
    Dev,
    NoMore,
}

#[derive(Clone, Copy)]
pub enum Prefix {
    None,
    Indent,
    Depth,
}

pub struct PrintConfig<'a> {
    /// Don't truncate dependencies that have already been displayed.
    pub all: bool,

    pub verbosity: Verbosity,
    pub direction: EdgeDirection,
    pub prefix: Prefix,

    // Is anyone using this? This is a carry-over from cargo-tree.
    // TODO: Open a github issue to discuss deprecation.
    pub format: &'a Pattern,

    pub charset: Charset,
    pub allow_partial_results: bool,
    pub include_tests: IncludeTests,
}

pub struct Node {
    id: PackageId,
    // TODO: Investigate why this was needed before the separation of printing
    // and graph traversal and if it should be added back.
    //pack: &'a Package,
}

pub struct Graph {
    graph: petgraph::Graph<Node, Kind>,
    nodes: HashMap<PackageId, NodeIndex>,
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

pub fn get_workspace(
    config: &Config,
    manifest_path: Option<PathBuf>,
) -> CargoResult<Workspace> {
    let root = match manifest_path {
        Some(path) => path,
        None => important_paths::find_root_manifest_for_wd(config.cwd())?,
    };
    Workspace::new(&root, config)
}

pub fn get_registry<'a>(
    config: &'a Config,
    package: &Package,
) -> CargoResult<PackageRegistry<'a>> {
    let mut registry = PackageRegistry::new(config)?;
    registry.add_sources(Some(package.package_id().source_id()))?;
    Ok(registry)
}

pub fn resolve<'a, 'cfg>(
    package_id: PackageId,
    registry: &mut PackageRegistry<'cfg>,
    ws: &'a Workspace<'cfg>,
    features: &[String],
    all_features: bool,
    no_default_features: bool,
) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let dev_deps = true; // TODO: Review this.
    let uses_default_features = !no_default_features;
    let opts = ResolveOpts::new(
        dev_deps,
        features,
        all_features,
        uses_default_features,
    );
    let prev = ops::load_pkg_lockfile(ws)?;
    let resolve = ops::resolve_with_previous(
        registry,
        ws,
        opts,
        prev.as_ref(),
        None,
        &[PackageIdSpec::from_package_id(package_id)],
        true,
    )?;
    let packages = ops::get_resolved_packages(
        &resolve,
        PackageRegistry::new(ws.config())?,
    )?;
    Ok((packages, resolve))
}

fn colorize(
    s: String,
    detection_status: &DetectionStatus,
) -> colored::ColoredString {
    match detection_status {
        DetectionStatus::NoneDetectedForbidsUnsafe => s.green(),
        DetectionStatus::NoneDetectedAllowsUnsafe => s.normal(),
        DetectionStatus::UnsafeDetected => s.red().bold(),
    }
}

pub fn run_scan_mode_default(
    config: &Config,
    ws: &Workspace,
    packages: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    pc: &PrintConfig,
    args: &Args,
) -> CliResult {
    let copt = build_compile_options(args, config);
    let rs_files_used = resolve_rs_file_deps(&copt, &ws).unwrap();
    if pc.verbosity == Verbosity::Verbose {
        // Print all .rs files found through the .d files, in sorted order.
        let mut paths = rs_files_used
            .iter()
            .map(std::borrow::ToOwned::to_owned)
            .collect::<Vec<PathBuf>>();
        paths.sort();
        paths
            .iter()
            .for_each(|p| println!("Used by build (sorted): {}", p.display()));
    }
    let mut progress = cargo::util::Progress::new("Scanning", config);
    let emoji_symbols = EmojiSymbols::new(pc.charset);
    let geiger_ctx = find_unsafe_in_packages(
        &packages,
        pc.allow_partial_results,
        pc.include_tests,
        ScanMode::Full,
        |i, count| -> CargoResult<()> { progress.tick(i, count) },
    );
    progress.clear();
    config.shell().status("Scanning", "done")?;

    println!();
    println!("Metric output format: x/y");
    println!("    x = unsafe code used by the build");
    println!("    y = total unsafe code found in the crate");
    println!();

    println!("Symbols: ");
    let forbids = "No `unsafe` usage found, declares #![forbid(unsafe_code)]";
    let unknown = "No `unsafe` usage found, missing #![forbid(unsafe_code)]";
    let guilty = "`unsafe` usage found";

    let shift_sequence = if emoji_symbols.will_output_emoji() {
        "\r\x1B[7C" // The radiation icon's Unicode width is 2,
                    // but by most terminals it seems to be rendered at width 1.
    } else {
        ""
    };

    println!(
        "    {: <2} = {}",
        emoji_symbols.emoji(SymbolKind::Lock),
        forbids
    );
    println!(
        "    {: <2} = {}",
        emoji_symbols.emoji(SymbolKind::QuestionMark),
        unknown
    );
    println!(
        "    {: <2}{} = {}",
        emoji_symbols.emoji(SymbolKind::Rads),
        shift_sequence,
        guilty
    );
    println!();

    println!(
        "{}",
        UNSAFE_COUNTERS_HEADER
            .iter()
            .map(|s| s.to_owned())
            .collect::<Vec<_>>()
            .join(" ")
            .bold()
    );
    println!();

    let tree_lines = walk_dependency_tree(root_pack_id, &graph, &pc);
    let mut total = CounterBlock::default();
    let mut total_unused = CounterBlock::default();
    let mut total_detection_status = DetectionStatus::NoneDetectedForbidsUnsafe;
    for tl in tree_lines {
        match tl {
            TextTreeLine::Package { id, tree_vines } => {
                let pack = packages.get_one(id).unwrap_or_else(|_| {
                    // TODO: Avoid panic, return Result.
                    panic!("Expected to find package by id: {}", id);
                });
                let pack_metrics = geiger_ctx
                    .pack_id_to_metrics
                    .get(&id)
                    .unwrap_or_else(|| {
                        // TODO: Avoid panic, return Result.
                        panic!(
                            "Failed to get unsafe counters for package: {}",
                            &id
                        )
                    });
                let unsafe_found = pack_metrics
                    .rs_path_to_metrics
                    .iter()
                    .filter(|(k, _)| rs_files_used.contains(k.as_path()))
                    .any(|(_, v)| v.metrics.counters.has_unsafe());

                // The crate level "forbids unsafe code" metric __used to__ only
                // depend on entry point source files that were __used by the
                // build__. This was too subtle in my opinion. For a crate to be
                // classified as forbidding unsafe code, all entry point source
                // files must declare `forbid(unsafe_code)`. Either a crate
                // forbids all unsafe code or it allows it _to some degree_.
                let crate_forbids_unsafe = pack_metrics
                    .rs_path_to_metrics
                    .iter()
                    .filter(|(_, v)| v.is_crate_entry_point)
                    .all(|(_, v)| v.metrics.forbids_unsafe);

                let detection_status =
                    match (unsafe_found, crate_forbids_unsafe) {
                        (false, true) => {
                            DetectionStatus::NoneDetectedForbidsUnsafe
                        }
                        (false, false) => {
                            DetectionStatus::NoneDetectedAllowsUnsafe
                        }
                        (true, _) => DetectionStatus::UnsafeDetected,
                    };

                for (k, v) in pack_metrics.rs_path_to_metrics.iter() {
                    let target = if rs_files_used.contains(k) {
                        &mut total
                    } else {
                        &mut total_unused
                    };
                    *target = target.clone() + v.metrics.counters.clone();
                    match (&detection_status, &total_detection_status) {
                        (DetectionStatus::UnsafeDetected, _) => {
                            total_detection_status =
                                DetectionStatus::UnsafeDetected;
                        }
                        (
                            DetectionStatus::NoneDetectedAllowsUnsafe,
                            DetectionStatus::NoneDetectedForbidsUnsafe,
                        ) => {
                            total_detection_status =
                                DetectionStatus::NoneDetectedForbidsUnsafe;
                        }
                        (_, _) => (),
                    }
                }

                let emoji_symbols = EmojiSymbols::new(pc.charset);
                let icon = match detection_status {
                    DetectionStatus::NoneDetectedForbidsUnsafe => {
                        emoji_symbols.emoji(SymbolKind::Lock)
                    }
                    DetectionStatus::NoneDetectedAllowsUnsafe => {
                        emoji_symbols.emoji(SymbolKind::QuestionMark)
                    }
                    DetectionStatus::UnsafeDetected => {
                        emoji_symbols.emoji(SymbolKind::Rads)
                    }
                };
                let pack_name = colorize(
                    format!(
                        "{}",
                        pc.format.display(&id, pack.manifest().metadata())
                    ),
                    &detection_status,
                );
                let unsafe_info = colorize(
                    table_row(&pack_metrics, &rs_files_used),
                    &detection_status,
                );
                let shift_chars = unsafe_info.chars().count() + 4;
                print!("{}  {: <2}", unsafe_info, icon);

                // Here comes some special control characters to position the cursor
                // properly for printing the last column containing the tree vines, after
                // the emoji icon. This is a workaround for a potential bug where the
                // radiation emoji will visually cover two characters in width but only
                // count as a single character if using the column formatting provided by
                // Rust. This could be unrelated to Rust and a quirk of this particular
                // symbol or something in the Terminal app on macOS.
                if emoji_symbols.will_output_emoji() {
                    print!("\r"); // Return the cursor to the start of the line.
                    print!("\x1B[{}C", shift_chars); // Move the cursor to the right so that it points to the icon character.
                }

                println!(" {}{}", tree_vines, pack_name);
            }
            TextTreeLine::ExtraDepsGroup { kind, tree_vines } => {
                let name = get_kind_group_name(kind);
                if name.is_none() {
                    continue;
                }
                let name = name.unwrap();

                // TODO: Fix the alignment on macOS (others too?)
                println!("{}{}{}", table_row_empty(), tree_vines, name);
            }
        }
    }
    println!("");
    println!(
        "{}",
        table_footer(total, total_unused, total_detection_status)
    );
    println!("");
    let scanned_files = geiger_ctx
        .pack_id_to_metrics
        .iter()
        .flat_map(|(_k, v)| v.rs_path_to_metrics.keys())
        .collect::<HashSet<&PathBuf>>();
    let used_but_not_scanned =
        rs_files_used.iter().filter(|p| !scanned_files.contains(p));
    for path in used_but_not_scanned {
        eprintln!(
            "WARNING: Dependency file was never scanned: {}",
            path.display()
        )
    }
    Ok(())
}

pub fn run_scan_mode_forbid_only(
    config: &Config,
    packages: &PackageSet,
    root_pack_id: PackageId,
    graph: &Graph,
    pc: &PrintConfig,
) -> CliResult {
    let emoji_symbols = EmojiSymbols::new(pc.charset);
    let mut progress = cargo::util::Progress::new("Scanning", config);
    let geiger_ctx = find_unsafe_in_packages(
        &packages,
        pc.allow_partial_results,
        pc.include_tests,
        ScanMode::EntryPointsOnly,
        |i, count| -> CargoResult<()> { progress.tick(i, count) },
    );
    progress.clear();
    config.shell().status("Scanning", "done")?;

    println!();

    println!("Symbols: ");
    let forbids = "All entry point .rs files declare #![forbid(unsafe_code)].";
    let unknown = "This crate may use unsafe code.";

    let sym_lock = emoji_symbols.emoji(SymbolKind::Lock);
    let sym_qmark = emoji_symbols.emoji(SymbolKind::QuestionMark);

    println!("    {: <2} = {}", sym_lock, forbids);
    println!("    {: <2} = {}", sym_qmark, unknown);
    println!();

    let tree_lines = walk_dependency_tree(root_pack_id, &graph, &pc);
    for tl in tree_lines {
        match tl {
            TextTreeLine::Package { id, tree_vines } => {
                let pack = packages.get_one(id).unwrap(); // FIXME
                let name = format_package_name(pack, pc.format);
                let pack_metrics = geiger_ctx.pack_id_to_metrics.get(&id);
                let package_forbids_unsafe = match pack_metrics {
                    None => false, // no metrics available, .rs parsing failed?
                    Some(pm) => pm
                        .rs_path_to_metrics
                        .iter()
                        .all(|(_k, v)| v.metrics.forbids_unsafe),
                };
                let (symbol, name) = if package_forbids_unsafe {
                    (&sym_lock, name.green())
                } else {
                    (&sym_qmark, name.red())
                };
                println!("{} {}{}", symbol, tree_vines, name);
            }
            TextTreeLine::ExtraDepsGroup { kind, tree_vines } => {
                let name = get_kind_group_name(kind);
                if name.is_none() {
                    continue;
                }
                let name = name.unwrap();
                // TODO: Fix the alignment on macOS (others too?)
                println!("  {}{}", tree_vines, name);
            }
        }
    }

    Ok(())
}

fn format_package_name(pack: &Package, pat: &Pattern) -> String {
    format!(
        "{}",
        pat.display(&pack.package_id(), pack.manifest().metadata())
    )
}

fn get_kind_group_name(k: Kind) -> Option<&'static str> {
    match k {
        Kind::Normal => None,
        Kind::Build => Some("[build-dependencies]"),
        Kind::Development => Some("[dev-dependencies]"),
    }
}

// ---------- END: Public items ----------

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

#[derive(Debug)]
enum RsResolveError {
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
struct RsFileMetricsWrapper {
    /// The information returned by the `geiger` crate for a `.rs` file.
    pub metrics: RsFileMetrics,

    /// All crate entry points must declare forbid(unsafe_code) to make it count
    /// for the crate as a whole. The `geiger` crate is decoupled from `cargo`
    /// and cannot know if a file is a crate entry point or not, so we add this
    /// information here.
    pub is_crate_entry_point: bool,
}

#[derive(Debug, Default)]
struct PackageMetrics {
    /// The key is the canonicalized path to the rs source file.
    pub rs_path_to_metrics: HashMap<PathBuf, RsFileMetricsWrapper>,
}

/// Provides a more terse and searchable name for the wrapped generic
/// collection.
struct GeigerContext {
    pack_id_to_metrics: HashMap<PackageId, PackageMetrics>,
}

/// Based on code from cargo-bloat. It seems weird that CompileOptions can be
/// constructed without providing all standard cargo options, TODO: Open an issue
/// in cargo?
fn build_compile_options<'a>(
    args: &'a Args,
    config: &'a Config,
) -> CompileOptions<'a> {
    let features = args
        .features
        .as_ref()
        .map(|f| f.clone())
        .unwrap_or_else(|| String::new())
        .split(' ')
        .map(str::to_owned)
        .collect::<Vec<String>>();
    let mut opt =
        CompileOptions::new(&config, CompileMode::Check { test: false })
            .unwrap();
    opt.features = features;
    opt.all_features = args.all_features;
    opt.no_default_features = args.no_default_features;

    // TODO: Investigate if this is relevant to cargo-geiger.
    //let mut bins = Vec::new();
    //let mut examples = Vec::new();
    // opt.release = args.release;
    // opt.target = args.target.clone();
    // if let Some(ref name) = args.bin {
    //     bins.push(name.clone());
    // } else if let Some(ref name) = args.example {
    //     examples.push(name.clone());
    // }
    // if args.bin.is_some() || args.example.is_some() {
    //     opt.filter = ops::CompileFilter::new(
    //         false,
    //         bins.clone(), false,
    //         Vec::new(), false,
    //         examples.clone(), false,
    //         Vec::new(), false,
    //         false,
    //     );
    // }

    opt
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

enum ScanMode {
    // The default scan mode, scan every .rs file.
    Full,

    // An optimization to allow skipping everything except the entry points.
    // This is only useful for the "--forbid-only" mode since that mode only
    // depends on entry point .rs files.
    EntryPointsOnly,
}

fn find_unsafe_in_packages<F>(
    packs: &PackageSet,
    allow_partial_results: bool,
    include_tests: IncludeTests,
    mode: ScanMode,
    mut progress_step: F,
) -> GeigerContext
where
    F: FnMut(usize, usize) -> CargoResult<()>,
{
    let mut pack_id_to_metrics = HashMap::new();
    let packs = packs.get_many(packs.package_ids()).unwrap();
    let pack_code_files: Vec<_> = find_rs_files_in_packages(&packs).collect();
    let pack_code_file_count = pack_code_files.len();
    for (i, (pack_id, rs_code_file)) in pack_code_files.into_iter().enumerate()
    {
        let (is_entry_point, p) = match rs_code_file {
            RsFile::LibRoot(pb) => (true, pb),
            RsFile::BinRoot(pb) => (true, pb),
            RsFile::CustomBuildRoot(pb) => (true, pb),
            RsFile::Other(pb) => (false, pb),
        };
        if let (false, ScanMode::EntryPointsOnly) = (is_entry_point, &mode) {
            continue;
        }
        match find_unsafe_in_file(&p, include_tests) {
            Err(e) => {
                if allow_partial_results {
                    eprintln!(
                        "Failed to parse file: {}, {:?} ",
                        &p.display(),
                        e
                    );
                } else {
                    panic!("Failed to parse file: {}, {:?} ", &p.display(), e);
                }
            }
            Ok(file_metrics) => {
                let package_metrics = pack_id_to_metrics
                    .entry(pack_id)
                    .or_insert_with(PackageMetrics::default);
                let wrapper = package_metrics
                    .rs_path_to_metrics
                    .entry(p)
                    .or_insert_with(RsFileMetricsWrapper::default);
                wrapper.metrics = file_metrics;
                wrapper.is_crate_entry_point = is_entry_point;
            }
        }
        let _ = progress_step(i, pack_code_file_count);
    }
    GeigerContext { pack_id_to_metrics }
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

struct TreeSymbols {
    down: &'static str,
    tee: &'static str,
    ell: &'static str,
    right: &'static str,
}

const UTF8_TREE_SYMBOLS: TreeSymbols = TreeSymbols {
    down: "│",
    tee: "├",
    ell: "└",
    right: "─",
};

const ASCII_TREE_SYMBOLS: TreeSymbols = TreeSymbols {
    down: "|",
    tee: "|",
    ell: "`",
    right: "-",
};

#[derive(Clone, Copy)]
enum SymbolKind {
    Lock = 0,
    QuestionMark = 1,
    Rads = 2,
}

struct EmojiSymbols {
    charset: Charset,
    emojis: [&'static str; 3],
    fallbacks: [colored::ColoredString; 3],
}

impl EmojiSymbols {
    pub fn new(charset: Charset) -> EmojiSymbols {
        Self {
            charset,
            emojis: ["🔒", "❓", "☢️"],
            fallbacks: [":)".green(), "?".normal(), "!".red().bold()],
        }
    }

    pub fn will_output_emoji(&self) -> bool {
        self.charset == Charset::Utf8
            && console::Term::stdout().features().wants_emoji()
    }

    pub fn emoji(&self, kind: SymbolKind) -> Box<dyn std::fmt::Display> {
        let idx = kind as usize;
        if self.will_output_emoji() {
            Box::new(self.emojis[idx])
        } else {
            Box::new(self.fallbacks[idx].clone())
        }
    }
}

/// Trigger a `cargo clean` + `cargo check` and listen to the cargo/rustc
/// communication to figure out which source files were used by the build.
fn resolve_rs_file_deps(
    copt: &CompileOptions,
    ws: &Workspace,
) -> Result<HashSet<PathBuf>, RsResolveError> {
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
    let mut hs = HashSet::<PathBuf>::new();
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
                hs.insert(p?);
            }
        }
    }
    for pb in rs_files {
        // rs_files must already be canonicalized
        hs.insert(pb);
    }
    Ok(hs)
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
        _on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        _on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
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

    /// Queried when queuing each unit of work. If it returns true, then the
    /// unit will always be rebuilt, independent of whether it needs to be.
    fn force_rebuild(&self, _unit: &Unit) -> bool {
        true // Overriding the default to force all units to be processed.
    }
}

/// A step towards decoupling some parts of the table-tree printing from the
/// dependency graph traversal.
enum TextTreeLine {
    /// A text line for a package
    Package { id: PackageId, tree_vines: String },
    /// There're extra dependencies comming and we should print a group header,
    /// eg. "[build-dependencies]".
    ExtraDepsGroup { kind: Kind, tree_vines: String },
}

/// To print the returned TextTreeLines in order are expectged to produce a nice
/// looking tree structure.
///
/// TODO: Return a impl Iterator<Item = TextTreeLine ... >
/// TODO: Consider separating the tree vine building from the tree traversal.
///
fn walk_dependency_tree(
    root_pack_id: PackageId,
    graph: &Graph,
    pc: &PrintConfig,
) -> Vec<TextTreeLine> {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];
    let node = &graph.graph[graph.nodes[&root_pack_id]];
    walk_dependency_node(
        node,
        graph,
        &mut visited_deps,
        &mut levels_continue,
        pc,
    )
}

fn walk_dependency_node(
    package: &Node,
    graph: &Graph,
    visited_deps: &mut HashSet<PackageId>,
    levels_continue: &mut Vec<bool>,
    pc: &PrintConfig,
) -> Vec<TextTreeLine> {
    let new = pc.all || visited_deps.insert(package.id);
    let tree_symbols = get_tree_symbols(pc.charset);
    let tree_vines = match pc.prefix {
        Prefix::Depth => format!("{} ", levels_continue.len()),
        Prefix::Indent => {
            let mut buf = String::new();
            if let Some((&last_continues, rest)) = levels_continue.split_last()
            {
                for &continues in rest {
                    let c = if continues { tree_symbols.down } else { " " };
                    buf.push_str(&format!("{}   ", c));
                }
                let c = if last_continues {
                    tree_symbols.tee
                } else {
                    tree_symbols.ell
                };
                buf.push_str(&format!("{0}{1}{1} ", c, tree_symbols.right));
            }
            buf
        }
        Prefix::None => "".into(),
    };

    let mut all_out = vec![TextTreeLine::Package {
        id: package.id,
        tree_vines,
    }];

    if !new {
        return all_out;
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
    let mut normal_out = walk_dependency_kind(
        Kind::Normal,
        &mut normal,
        graph,
        visited_deps,
        levels_continue,
        pc,
    );
    let mut build_out = walk_dependency_kind(
        Kind::Build,
        &mut build,
        graph,
        visited_deps,
        levels_continue,
        pc,
    );
    let mut dev_out = walk_dependency_kind(
        Kind::Development,
        &mut development,
        graph,
        visited_deps,
        levels_continue,
        pc,
    );
    all_out.append(&mut normal_out);
    all_out.append(&mut build_out);
    all_out.append(&mut dev_out);
    all_out
}

fn walk_dependency_kind(
    kind: Kind,
    deps: &mut Vec<&Node>,
    graph: &Graph,
    visited_deps: &mut HashSet<PackageId>,
    levels_continue: &mut Vec<bool>,
    pc: &PrintConfig,
) -> Vec<TextTreeLine> {
    if deps.is_empty() {
        return Vec::new();
    }

    // Resolve uses Hash data types internally but we want consistent output ordering
    deps.sort_by_key(|n| n.id);

    let tree_symbols = get_tree_symbols(pc.charset);
    let mut output = Vec::new();
    if let Prefix::Indent = pc.prefix {
        match kind {
            Kind::Normal => (),
            _ => {
                let mut tree_vines = String::new();
                for &continues in &**levels_continue {
                    let c = if continues { tree_symbols.down } else { " " };
                    tree_vines.push_str(&format!("{}   ", c));
                }
                output.push(TextTreeLine::ExtraDepsGroup { kind, tree_vines });
            }
        }
    }

    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        output.append(&mut walk_dependency_node(
            dependency,
            graph,
            visited_deps,
            levels_continue,
            pc,
        ));
        levels_continue.pop();
    }
    output
}

enum DetectionStatus {
    NoneDetectedForbidsUnsafe,
    NoneDetectedAllowsUnsafe,
    UnsafeDetected,
}

fn get_tree_symbols(cs: Charset) -> TreeSymbols {
    match cs {
        Charset::Utf8 => UTF8_TREE_SYMBOLS,
        Charset::Ascii => ASCII_TREE_SYMBOLS,
    }
}

// TODO: use a table library, or factor the tableness out in a smarter way. This
// is probably easier now when the tree formatting is separated from the tree
// traversal.
const UNSAFE_COUNTERS_HEADER: [&str; 6] = [
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

fn table_row(pms: &PackageMetrics, rs_files_used: &HashSet<PathBuf>) -> String {
    let mut used = CounterBlock::default();
    let mut not_used = CounterBlock::default();
    for (k, v) in pms.rs_path_to_metrics.iter() {
        let target = if rs_files_used.contains(k) {
            &mut used
        } else {
            &mut not_used
        };
        *target = target.clone() + v.metrics.counters.clone();
    }
    let fmt = |used: &Count, not_used: &Count| {
        format!("{}/{}", used.unsafe_, used.unsafe_ + not_used.unsafe_)
    };
    format!(
        "{: <10} {: <12} {: <6} {: <7} {: <7}",
        fmt(&used.functions, &not_used.functions),
        fmt(&used.exprs, &not_used.exprs),
        fmt(&used.item_impls, &not_used.item_impls),
        fmt(&used.item_traits, &not_used.item_traits),
        fmt(&used.methods, &not_used.methods),
    )
}

fn table_footer(
    used: CounterBlock,
    not_used: CounterBlock,
    status: DetectionStatus,
) -> colored::ColoredString {
    let fmt = |used: &Count, not_used: &Count| {
        format!("{}/{}", used.unsafe_, used.unsafe_ + not_used.unsafe_)
    };
    let output = format!(
        "{: <10} {: <12} {: <6} {: <7} {: <7}",
        fmt(&used.functions, &not_used.functions),
        fmt(&used.exprs, &not_used.exprs),
        fmt(&used.item_impls, &not_used.item_impls),
        fmt(&used.item_traits, &not_used.item_traits),
        fmt(&used.methods, &not_used.methods),
    );
    colorize(output, &status)
}

pub fn find_rs_files_in_dir(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let walker = WalkDir::new(dir).into_iter();
    walker.filter_map(|entry| {
        let entry = entry.expect("walkdir error."); // TODO: Return result.
        if !is_file_with_ext(&entry, "rs") {
            return None;
        }
        Some(
            entry
                .path()
                .canonicalize()
                .expect("Error converting to canonical path"),
        ) // TODO: Return result.
    })
}
