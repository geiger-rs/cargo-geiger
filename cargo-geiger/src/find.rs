use crate::report::UnsafeInfo;
use crate::rs_file::{
    into_rs_code_file, is_file_with_ext, PackageMetrics, RsFile,
    RsFileMetricsWrapper,
};
use crate::scan::ScanMode;

use cargo::core::package::PackageSet;
use cargo::core::{Package, PackageId};
use cargo::util::CargoResult;
use geiger::find_unsafe_in_file;
use geiger::{CounterBlock, IncludeTests};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Provides a more terse and searchable name for the wrapped generic
/// collection.
pub struct GeigerContext {
    pub pack_id_to_metrics: HashMap<PackageId, PackageMetrics>,
}

pub fn find_unsafe_in_packages<F>(
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

fn find_rs_files_in_dir(dir: &Path) -> impl Iterator<Item = PathBuf> {
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

fn find_rs_files_in_packages<'a>(
    packs: &'a [&Package],
) -> impl Iterator<Item = (PackageId, RsFile)> + 'a {
    packs.iter().flat_map(|pack| {
        find_rs_files_in_package(pack)
            .into_iter()
            .map(move |path| (pack.package_id(), path))
    })
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

    for (path_buf, rs_file_metrics_wrapper) in &pack_metrics.rs_path_to_metrics {
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

#[cfg(test)]
mod tests {
    use crate::{
        find::unsafe_stats,
        report::UnsafeInfo,
        rs_file::{PackageMetrics, RsFileMetricsWrapper},
    };
    use geiger::Count;
    use std::{
        collections::HashSet,
        path::PathBuf,
    };

    #[test]
    fn unsafe_stats_from_nothing_are_empty() {
        let stats = unsafe_stats(&Default::default(), &Default::default());
        let expected = UnsafeInfo { forbids_unsafe: true, ..Default::default() };
        assert_eq!(stats, expected);
    }

    #[test]
    fn unsafe_stats_report_forbid_unsafe_as_true_if_all_entry_points_forbid_unsafe() {
        let metrics = metrics_from_iter(vec![
            (
                "foo.rs",
                MetricsBuilder::default().forbids_unsafe(true).is_entry_point(true).build(),
            ),
        ]);
        let stats = unsafe_stats(&metrics, &set_of_paths(&["foo.rs"]));
        assert!(stats.forbids_unsafe)
    }

    #[test]
    fn unsafe_stats_report_forbid_unsafe_as_false_if_one_entry_point_allows_unsafe() {
        let metrics = metrics_from_iter(vec![
            (
                "foo.rs",
                MetricsBuilder::default().forbids_unsafe(true).is_entry_point(true).build(),
            ),
            (
                "bar.rs",
                MetricsBuilder::default().forbids_unsafe(false).is_entry_point(true).build(),
            ),
        ]);
        let stats = unsafe_stats(&metrics, &set_of_paths(&["foo.rs", "bar.rs"]));
        assert!(!stats.forbids_unsafe)
    }

    #[test]
    fn unsafe_stats_accumulate_counters() {
        let metrics = metrics_from_iter(vec![
            (
                "foo.rs",
                MetricsBuilder::default().functions(2, 1).build(),
            ),
            (
                "bar.rs",
                MetricsBuilder::default().functions(5, 3).build(),
            ),
            (
                "baz.rs",
                MetricsBuilder::default().functions(20, 10).build(),
            ),
            (
                "quux.rs",
                MetricsBuilder::default().functions(200, 100).build(),
            ),
        ]);
        let stats = unsafe_stats(&metrics, &set_of_paths(&["foo.rs", "bar.rs"]));
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
            rs_path_to_metrics: it.into_iter().map(|(p, m)| (p.into(), m)).collect(),
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

        fn is_entry_point(mut self, yes: bool) -> Self {
            self.inner.is_crate_entry_point = yes;
            self
        }

        fn build(self) -> RsFileMetricsWrapper {
            self.inner
        }
    }
}
