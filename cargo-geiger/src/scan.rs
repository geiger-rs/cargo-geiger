mod default;
mod find;
mod forbid;
mod rs_file;

use crate::args::Args;
use crate::format::print_config::PrintConfig;
use crate::graph::Graph;
use crate::mapping::{
    CargoMetadataParameters, ToCargoGeigerDependencyKind,
    ToCargoGeigerPackageId,
};

pub use rs_file::RsFileMetricsWrapper;

use default::scan_unsafe;
use forbid::scan_forbid_unsafe;

use cargo::core::Workspace;
use cargo::{CliResult, Config};
use cargo_geiger_serde::{CounterBlock, PackageInfo, UnsafeInfo};
use cargo_metadata::PackageId;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Provides a more terse and searchable name for the wrapped generic
/// collection.
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
) -> CliResult {
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
    pack_metrics: &PackageMetrics,
    rs_files_used: &HashSet<PathBuf>,
) -> UnsafeInfo {
    // The crate level "forbids unsafe code" metric __used to__ only
    // depend on entry point source files that were __used by the
    // build__. This was too subtle in my opinion. For a crate to be
    // classified as forbidding unsafe code, all entry point source
    // files must declare `forbid(unsafe_code)`. Either a crate
    // forbids all unsafe code or it allows it _to some degree_.
    let forbids_unsafe = pack_metrics
        .rs_path_to_metrics
        .iter()
        .filter(|(_, v)| v.is_crate_entry_point)
        .all(|(_, v)| v.metrics.forbids_unsafe);

    let mut used = CounterBlock::default();
    let mut unused = CounterBlock::default();

    for (path_buf, rs_file_metrics_wrapper) in &pack_metrics.rs_path_to_metrics
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
        .flat_map(|(_, v)| v.rs_path_to_metrics.keys())
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

    while !indices.is_empty() {
        let i = indices.pop().unwrap();
        let package_id = graph.graph[i].clone();
        let mut package = PackageInfo::new(
            package_id
                .to_cargo_geiger_package_id(cargo_metadata_parameters.metadata),
        );
        for edge in graph.graph.edges(i) {
            let dep_index = edge.target();
            if visited.insert(dep_index) {
                indices.push(dep_index);
            }
            let dep = graph.graph[dep_index]
                .to_cargo_geiger_package_id(cargo_metadata_parameters.metadata);

            package.add_dependency(
                dep,
                edge.weight().to_cargo_geiger_dependency_kind(),
            );
        }
        match geiger_context.package_id_to_metrics.get(&package_id) {
            Some(m) => package_metrics.push((package, Some(m.clone()))),
            None => {
                eprintln!(
                    "WARNING: No metrics found for package: {}",
                    package_id
                );
                package_metrics.push((package, None))
            }
        }
    }

    package_metrics
}

#[cfg(test)]
mod scan_tests {
    use super::*;

    use crate::scan::PackageMetrics;
    use rs_file::RsFileMetricsWrapper;

    use cargo_geiger_serde::{Count, UnsafeInfo};
    use rstest::*;
    use std::{collections::HashSet, path::PathBuf};

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
