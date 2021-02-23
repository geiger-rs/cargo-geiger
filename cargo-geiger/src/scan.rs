mod default;
mod find;
mod forbid;
mod rs_file;

use crate::args::Args;
use crate::format::print_config::PrintConfig;
use crate::graph::Graph;
use crate::mapping::{
    geiger::ToCargoGeigerDependencyKind, CargoMetadataParameters,
    ToCargoGeigerPackageId,
};

pub use rs_file::RsFileMetricsWrapper;

use default::scan_unsafe;
use forbid::scan_forbid_unsafe;

use cargo::core::Workspace;
use cargo::{CliError, Config};
use cargo_geiger_serde::{
    CounterBlock, DependencyKind, PackageInfo, UnsafeInfo,
};
use cargo_metadata::PackageId;
use krates::NodeId;
use petgraph::prelude::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub struct FoundWarningsError {
    pub warning_count: u64,
}

impl Error for FoundWarningsError {}

/// Forward Display to Debug.
impl fmt::Display for FoundWarningsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

pub struct ScanResult {
    pub scan_output_lines: Vec<String>,
    pub warning_count: u64,
}

/// Provides a more terse and searchable name for the wrapped generic
/// collection.
#[derive(Default)]
pub struct GeigerContext {
    pub package_id_to_metrics: HashMap<PackageId, PackageMetrics>,
}

#[derive(Clone, Debug, Default)]
pub struct PackageMetrics {
    /// The key is the canonicalized path to the rs source file.
    pub rs_path_to_metrics: HashMap<PathBuf, RsFileMetricsWrapper>,
}

pub enum ScanMode {
    // An optimization to allow skipping everything except the entry points.
    // This is only useful for the "--forbid-only" mode since that mode only
    // depends on entry point .rs files.
    EntryPointsOnly,

    // The default scan mode, scan every .rs file.
    Full,
}

pub struct ScanParameters<'a> {
    pub args: &'a Args,
    pub config: &'a Config,
    pub print_config: &'a PrintConfig,
}

pub fn scan(
    args: &Args,
    cargo_metadata_parameters: &CargoMetadataParameters,
    config: &Config,
    graph: &Graph,
    root_package_id: PackageId,
    workspace: &Workspace,
) -> Result<ScanResult, CliError> {
    let print_config = PrintConfig::new(args)?;

    let scan_parameters = ScanParameters {
        args: &args,
        config: &config,
        print_config: &print_config,
    };

    if args.forbid_only {
        scan_forbid_unsafe(
            cargo_metadata_parameters,
            &graph,
            root_package_id,
            &scan_parameters,
        )
    } else {
        scan_unsafe(
            cargo_metadata_parameters,
            &graph,
            root_package_id,
            &scan_parameters,
            workspace,
        )
    }
}

pub fn unsafe_stats(
    package_metrics: &PackageMetrics,
    rs_files_used: &HashSet<PathBuf>,
) -> UnsafeInfo {
    // The crate level "forbids unsafe code" metric __used to__ only
    // depend on entry point source files that were __used by the
    // build__. This was too subtle in my opinion. For a crate to be
    // classified as forbidding unsafe code, all entry point source
    // files must declare `forbid(unsafe_code)`. Either a crate
    // forbids all unsafe code or it allows it _to some degree_.
    let forbids_unsafe = package_metrics
        .rs_path_to_metrics
        .iter()
        .filter(|(_, v)| v.is_crate_entry_point)
        .all(|(_, v)| v.metrics.forbids_unsafe);

    let mut used = CounterBlock::default();
    let mut unused = CounterBlock::default();

    for (path_buf, rs_file_metrics_wrapper) in
        &package_metrics.rs_path_to_metrics
    {
        let target = if rs_files_used.contains(path_buf) {
            &mut used
        } else {
            &mut unused
        };
        *target += rs_file_metrics_wrapper.metrics.counters.clone();
    }
    UnsafeInfo {
        used,
        unused,
        forbids_unsafe,
    }
}

struct ScanDetails {
    rs_files_used: HashSet<PathBuf>,
    geiger_context: GeigerContext,
}

fn construct_rs_files_used_lines(
    rs_files_used: &HashSet<PathBuf>,
) -> Vec<String> {
    // Print all .rs files found through the .d files, in sorted order.
    let mut paths = rs_files_used
        .iter()
        .map(std::borrow::ToOwned::to_owned)
        .collect::<Vec<PathBuf>>();

    paths.sort();

    paths
        .iter()
        .map(|p| format!("Used by build (sorted): {}", p.display()))
        .collect::<Vec<String>>()
}

fn list_files_used_but_not_scanned(
    geiger_context: &GeigerContext,
    rs_files_used: &HashSet<PathBuf>,
) -> Vec<PathBuf> {
    let scanned_files = geiger_context
        .package_id_to_metrics
        .iter()
        .flat_map(|(_, package_metrics)| {
            package_metrics.rs_path_to_metrics.keys()
        })
        .collect::<HashSet<&PathBuf>>();

    rs_files_used
        .iter()
        .cloned()
        .filter(|p| !scanned_files.contains(p))
        .collect()
}

fn package_metrics(
    cargo_metadata_parameters: &CargoMetadataParameters,
    geiger_context: &GeigerContext,
    graph: &Graph,
    root_package_id: PackageId,
) -> Vec<(PackageInfo, Option<PackageMetrics>)> {
    let mut package_metrics =
        Vec::<(PackageInfo, Option<PackageMetrics>)>::new();
    let root_index = graph.nodes[&root_package_id];
    let mut indices = vec![root_index];
    let mut visited = HashSet::new();

    while let Some(index) = indices.pop() {
        let package_id = graph.graph[index].clone();

        if let Some(package) = package_id
            .to_cargo_geiger_package_id(cargo_metadata_parameters.metadata)
        {
            let mut package_info = PackageInfo::new(package);

            for edge in graph.graph.edges(index) {
                let dep_index = edge.target();

                let dependency_kind_option =
                    edge.weight().to_cargo_geiger_dependency_kind();

                add_dependency_to_package_info(
                    cargo_metadata_parameters,
                    dep_index,
                    dependency_kind_option,
                    graph,
                    &mut indices,
                    &mut package_info,
                    &mut visited,
                );
            }

            match geiger_context.package_id_to_metrics.get(&package_id) {
                Some(m) => {
                    package_metrics.push((package_info, Some(m.clone())))
                }
                None => {
                    eprintln!(
                        "WARNING: No metrics found for package: {}",
                        package_id
                    );
                    package_metrics.push((package_info, None))
                }
            }
        }
    }

    package_metrics
}

fn add_dependency_to_package_info(
    cargo_metadata_parameters: &CargoMetadataParameters,
    dependency_index: NodeId,
    dependency_kind_option: Option<DependencyKind>,
    graph: &Graph,
    indices: &mut Vec<NodeIndex>,
    package_info: &mut PackageInfo,
    visited: &mut HashSet<NodeId>,
) {
    if visited.insert(dependency_index) {
        indices.push(dependency_index);
    }

    let dependency_package_id_option = graph.graph[dependency_index]
        .to_cargo_geiger_package_id(cargo_metadata_parameters.metadata);

    match (dependency_package_id_option, dependency_kind_option) {
        (Some(dependency_package_id), Some(dependency_kind)) => {
            package_info.add_dependency(dependency_package_id, dependency_kind);
        }
        (Some(dependency_package_id), None) => {
            eprintln!(
                "Failed to add dependency for: {} {:?}",
                dependency_package_id.name, dependency_package_id.version
            )
        }
        _ => {
            eprintln!(
                "Error converting: {} to Cargo Geiger Package Id",
                graph.graph[dependency_index]
            )
        }
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;

    use crate::scan::PackageMetrics;
    use rs_file::RsFileMetricsWrapper;

    use crate::lib_tests::construct_krates_and_metadata;
    use cargo_geiger_serde::{Count, Source, UnsafeInfo};
    use rstest::*;
    use semver::Version;
    use std::{collections::HashSet, path::PathBuf};
    use url::Url;

    #[rstest(
        input_dependency_kind_option,
        expected_package_info_dependency_length,
        case(
            Some(DependencyKind::Normal),
            1,
        ),
        case(
            None,
            0
        )
    )]
    fn add_dependency_to_package_info_test(
        input_dependency_kind_option: Option<DependencyKind>,
        expected_package_info_dependency_length: usize
    ) {
        let (krates, metadata) = construct_krates_and_metadata();
        let package_id = metadata.root_package().unwrap().id.clone();

        let cargo_metadata_parameters = CargoMetadataParameters {
            krates: &krates,
            metadata: &metadata,
        };

        let mut graph = Graph {
            graph: Default::default(),
            nodes: Default::default(),
        };
        graph.graph.add_node(package_id);

        let mut package_info = PackageInfo {
            id: cargo_geiger_serde::PackageId {
                name: String::from("package_id"),
                version: Version {
                    major: 0,
                    minor: 0,
                    patch: 0,
                    pre: vec![],
                    build: vec![],
                },
                source: Source::Path(
                    Url::parse(
                        "https://github.com/rust-secure-code/cargo-geiger",
                    )
                    .unwrap(),
                ),
            },
            dependencies: Default::default(),
            dev_dependencies: Default::default(),
            build_dependencies: Default::default(),
        };

        let mut indices = vec![];
        let mut visited = HashSet::new();

        let dependency_index = NodeIndex::new(0);

        add_dependency_to_package_info(
            &cargo_metadata_parameters,
            NodeIndex::new(0),
            input_dependency_kind_option,
            &graph,
            &mut indices,
            &mut package_info,
            &mut visited,
        );

        assert_eq!(visited, vec![dependency_index].iter().cloned().collect());
        assert_eq!(package_info.dependencies.len(), expected_package_info_dependency_length)
    }

    #[rstest]
    fn construct_rs_files_used_lines_test() {
        let mut rs_files_used = HashSet::<PathBuf>::new();

        rs_files_used.insert(PathBuf::from("b/path.rs"));
        rs_files_used.insert(PathBuf::from("a/path.rs"));
        rs_files_used.insert(PathBuf::from("c/path.rs"));

        let rs_files_used_lines = construct_rs_files_used_lines(&rs_files_used);

        assert_eq!(
            rs_files_used_lines,
            vec![
                String::from("Used by build (sorted): a/path.rs"),
                String::from("Used by build (sorted): b/path.rs"),
                String::from("Used by build (sorted): c/path.rs"),
            ]
        );
    }

    #[rstest(
        input_rs_path_to_metrics_vec,
        input_rs_files_used_vec,
        expected_files_used_but_not_scanned,
        case(
            vec![(
                PathBuf::from("third/file/path.rs"),
                RsFileMetricsWrapper {
                    metrics: Default::default(),
                    is_crate_entry_point: false,
                },
            )],
            vec![
                PathBuf::from("first/file/path.rs"),
                PathBuf::from("second/file/path.rs"),
                PathBuf::from("third/file/path.rs"),
            ],
            vec![
                PathBuf::from("first/file/path.rs"),
                PathBuf::from("second/file/path.rs")
            ]
        ),
        case(
            vec![(
                PathBuf::from("first/file/path.rs"),
                RsFileMetricsWrapper {
                    metrics: Default::default(),
                    is_crate_entry_point: false,
                }),
                (
                PathBuf::from("second/file/path.rs"),
                RsFileMetricsWrapper {
                metrics: Default::default(),
                is_crate_entry_point: false,
                }),
                (PathBuf::from("third/file/path.rs"),
                RsFileMetricsWrapper {
                    metrics: Default::default(),
                    is_crate_entry_point: false,
                }
            )],
            vec![
                PathBuf::from("first/file/path.rs"),
                PathBuf::from("second/file/path.rs"),
                PathBuf::from("third/file/path.rs"),
            ],
            vec![
            ]
        )
    )]
    fn list_files_used_but_not_scanned_test(
        input_rs_path_to_metrics_vec: Vec<(PathBuf, RsFileMetricsWrapper)>,
        input_rs_files_used_vec: Vec<PathBuf>,
        expected_files_used_but_not_scanned: Vec<PathBuf>,
    ) {
        let (_, metadata) = construct_krates_and_metadata();
        let package_id = metadata.root_package().unwrap().id.clone();

        let rs_path_to_metrics: HashMap<PathBuf, RsFileMetricsWrapper> =
            input_rs_path_to_metrics_vec.iter().cloned().collect();

        let geiger_context = GeigerContext {
            package_id_to_metrics: vec![(
                package_id,
                PackageMetrics { rs_path_to_metrics },
            )]
            .iter()
            .cloned()
            .collect(),
        };

        let rs_files_used = input_rs_files_used_vec.iter().cloned().collect();

        let mut files_used_but_not_scanned =
            list_files_used_but_not_scanned(&geiger_context, &rs_files_used);

        files_used_but_not_scanned.sort();

        assert_eq!(
            files_used_but_not_scanned,
            expected_files_used_but_not_scanned
        )
    }

    #[rstest]
    fn unsafe_stats_from_nothing_are_empty() {
        let stats = unsafe_stats(&Default::default(), &Default::default());
        let expected = UnsafeInfo {
            forbids_unsafe: true,
            ..Default::default()
        };
        assert_eq!(stats, expected);
    }

    #[rstest]
    fn unsafe_stats_report_forbid_unsafe_as_true_if_all_entry_points_forbid_unsafe(
    ) {
        let metrics = metrics_from_iter(vec![(
            "foo.rs",
            MetricsBuilder::default()
                .forbids_unsafe(true)
                .set_is_crate_entry_point(true)
                .build(),
        )]);
        let stats = unsafe_stats(&metrics, &set_of_paths(&["foo.rs"]));
        assert!(stats.forbids_unsafe)
    }

    #[rstest]
    fn unsafe_stats_report_forbid_unsafe_as_false_if_one_entry_point_allows_unsafe(
    ) {
        let metrics = metrics_from_iter(vec![
            (
                "foo.rs",
                MetricsBuilder::default()
                    .forbids_unsafe(true)
                    .set_is_crate_entry_point(true)
                    .build(),
            ),
            (
                "bar.rs",
                MetricsBuilder::default()
                    .forbids_unsafe(false)
                    .set_is_crate_entry_point(true)
                    .build(),
            ),
        ]);
        let stats =
            unsafe_stats(&metrics, &set_of_paths(&["foo.rs", "bar.rs"]));
        assert!(!stats.forbids_unsafe)
    }

    #[rstest]
    fn unsafe_stats_accumulate_counters() {
        let metrics = metrics_from_iter(vec![
            ("foo.rs", MetricsBuilder::default().functions(2, 1).build()),
            ("bar.rs", MetricsBuilder::default().functions(5, 3).build()),
            (
                "baz.rs",
                MetricsBuilder::default().functions(20, 10).build(),
            ),
            (
                "quux.rs",
                MetricsBuilder::default().functions(200, 100).build(),
            ),
        ]);
        let stats =
            unsafe_stats(&metrics, &set_of_paths(&["foo.rs", "bar.rs"]));
        assert_eq!(stats.used.functions.safe, 7);
        assert_eq!(stats.used.functions.unsafe_, 4);
        assert_eq!(stats.unused.functions.safe, 220);
        assert_eq!(stats.unused.functions.unsafe_, 110);
    }

    fn metrics_from_iter<I, P>(it: I) -> PackageMetrics
    where
        I: IntoIterator<Item = (P, RsFileMetricsWrapper)>,
        P: Into<PathBuf>,
    {
        PackageMetrics {
            rs_path_to_metrics: it
                .into_iter()
                .map(|(p, m)| (p.into(), m))
                .collect(),
        }
    }

    fn set_of_paths<I>(it: I) -> HashSet<PathBuf>
    where
        I: IntoIterator,
        I::Item: Into<PathBuf>,
    {
        it.into_iter().map(Into::into).collect()
    }

    #[derive(Default)]
    struct MetricsBuilder {
        inner: RsFileMetricsWrapper,
    }

    impl MetricsBuilder {
        fn forbids_unsafe(mut self, yes: bool) -> Self {
            self.inner.metrics.forbids_unsafe = yes;
            self
        }

        fn functions(mut self, safe: u64, unsafe_: u64) -> Self {
            self.inner.metrics.counters.functions = Count { safe, unsafe_ };
            self
        }

        fn set_is_crate_entry_point(mut self, yes: bool) -> Self {
            self.inner.is_crate_entry_point = yes;
            self
        }

        fn build(self) -> RsFileMetricsWrapper {
            self.inner
        }
    }
}
