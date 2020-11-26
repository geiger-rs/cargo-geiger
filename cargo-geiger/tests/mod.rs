#![forbid(unsafe_code)]
#![forbid(warnings)]

use assert_cmd::prelude::*;
use cargo_geiger_serde::{
    Count, CounterBlock, PackageId, PackageInfo, QuickReportEntry,
    QuickSafetyReport, ReportEntry, SafetyReport, Source, UnsafeInfo,
};
use insta::assert_snapshot;
use rstest::rstest;
use semver::Version;
use tempfile::TempDir;
use url::Url;

use std::collections::{HashMap, HashSet};
use std::env;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[rstest(
    name,
    case("test1_package_with_no_deps"),
    case("test2_package_with_shallow_deps"),
    case("test3_package_with_nested_deps"),
    case("test4_workspace_with_top_level_package"),
    case("test5_workspace_with_virtual_manifest"),
    case("test6_cargo_lock_out_of_date"),
    case("test7_package_with_patched_dep")
)]
fn test_package(name: &str) {
    better_panic::install();

    let result = run_geiger(name);

    let stderr_filename = format!("{}.stderr", name);
    let stderr = String::from_utf8(result.stderr)
        .expect("output should have been valid utf-8");

    if !stderr.is_empty() {
        let re = regex::Regex::new(r"`([^`]+).toml`").unwrap();
        let stderr = re.replace(&stderr, "`{MANIFEST_PATH}`");
        assert_snapshot!(stderr_filename, stderr);
    }

    let stdout_filename = format!("{}.stdout", name);
    let stdout = String::from_utf8(result.stdout)
        .expect("output should have been valid utf-8");
    assert_snapshot!(stdout_filename, stdout);

    if stderr.is_empty() {
        assert!(result.status.success(), "`cargo-geiger` failed");
    }
}

#[test]
fn serialize_test1_report() {
    Test1.run();
}

#[test]
fn serialize_test2_report() {
    Test2.run();
}

#[test]
fn serialize_test3_report() {
    Test3.run();
}

#[test]
fn serialize_test4_report() {
    Test4.run();
}

#[test]
fn serialize_test6_report() {
    Test6.run();
}

#[test]
fn serialize_test7_report() {
    Test7.run();
}

#[test]
fn serialize_test1_quick_report() {
    Test1.run_quick();
}

#[test]
fn serialize_test2_quick_report() {
    Test2.run_quick();
}

#[test]
fn serialize_test3_quick_report() {
    Test3.run_quick();
}

#[test]
fn serialize_test4_quick_report() {
    Test4.run_quick();
}

#[test]
fn serialize_test6_quick_report() {
    Test6.run_quick();
}

#[test]
fn serialize_test7_quick_report() {
    Test7.run_quick();
}

trait Test {
    const NAME: &'static str;

    fn expected_report(&self, cx: &Context) -> SafetyReport;
    fn expected_report_entry(&self, cx: &Context) -> ReportEntry;

    fn expected_quick_report(&self, cx: &Context) -> QuickSafetyReport {
        to_quick_report(self.expected_report(cx))
    }

    fn run(&self) {
        let (output, cx) = run_geiger_json(Self::NAME);
        assert!(output.status.success());
        let actual =
            serde_json::from_slice::<SafetyReport>(&output.stdout).unwrap();
        assert_eq!(actual, self.expected_report(&cx));
    }

    fn run_quick(&self) {
        let (output, cx) = run_geiger_json_quick(Self::NAME);
        assert!(output.status.success());
        let actual =
            serde_json::from_slice::<QuickSafetyReport>(&output.stdout)
                .unwrap();
        assert_eq!(actual, self.expected_quick_report(&cx));
    }
}

struct Test1;

impl Test for Test1 {
    const NAME: &'static str = "test1_package_with_no_deps";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        single_entry_safety_report(self.expected_report_entry(cx))
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo::new(make_package_id(cx, Self::NAME)),
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 1,
                    },
                    exprs: Count {
                        safe: 4,
                        unsafe_: 2,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

struct Test2;

impl Test for Test2 {
    const NAME: &'static str = "test2_package_with_shallow_deps";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(&mut report, Test1.expected_report(cx));
        merge_test_reports(&mut report, external::ref_slice_safety_report());
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![
                    make_package_id(cx, Test1::NAME),
                    external::ref_slice_package_id(),
                ]),
                ..PackageInfo::new(make_package_id(cx, Self::NAME))
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 0,
                        unsafe_: 1,
                    },
                    exprs: Count {
                        safe: 0,
                        unsafe_: 4,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

struct Test3;

impl Test for Test3 {
    const NAME: &'static str = "test3_package_with_nested_deps";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(&mut report, external::itertools_safety_report());
        merge_test_reports(&mut report, external::doc_comment_safety_report());
        merge_test_reports(&mut report, Test2.expected_report(cx));
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![
                    make_package_id(cx, Test2::NAME),
                    external::itertools_package_id(),
                    external::doc_comment_package_id(),
                ]),
                ..PackageInfo::new(make_package_id(cx, Self::NAME))
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 6,
                        unsafe_: 1,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

struct Test4;

impl Test for Test4 {
    const NAME: &'static str = "test4_workspace_with_top_level_package";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(&mut report, Test1.expected_report(cx));
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![make_package_id(cx, Test1::NAME)]),
                ..PackageInfo::new(make_package_id(cx, Self::NAME))
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                unused: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 1,
                        unsafe_: 1,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

struct Test6;

impl Test for Test6 {
    const NAME: &'static str = "test6_cargo_lock_out_of_date";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(
            &mut report,
            external::generational_arena_safety_report(),
        );
        merge_test_reports(&mut report, external::idna_safety_report());
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![
                    external::generational_arena_package_id(),
                    external::idna_package_id(),
                ]),
                ..PackageInfo::new(make_package_id(cx, Self::NAME))
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                forbids_unsafe: true,
                ..Default::default()
            },
        }
    }
}

struct Test7;

impl Test for Test7 {
    const NAME: &'static str = "test7_package_with_patched_dep";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(&mut report, external::num_cpus_safety_report(cx));
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![external::num_cpus_package_id(cx)]),
                ..PackageInfo::new(make_package_id(cx, Self::NAME))
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                forbids_unsafe: true,
                ..Default::default()
            },
        }
    }
}

fn run_geiger(test_name: &str) -> Output {
    run_geiger_with(test_name, None::<&str>).0
}

fn run_geiger_json(test_name: &str) -> (Output, Context) {
    run_geiger_with(test_name, &["--json"])
}

fn run_geiger_json_quick(test_name: &str) -> (Output, Context) {
    run_geiger_with(test_name, &["--forbid-only", "--json"])
}

fn run_geiger_with<I>(test_name: &str, extra_args: I) -> (Output, Context)
where
    I: IntoIterator,
    I::Item: AsRef<std::ffi::OsStr>,
{
    let cx = Context::new();
    let output = Command::cargo_bin("cargo-geiger")
        .unwrap()
        .arg("geiger")
        .arg("--color=never")
        .arg("--quiet")
        .arg("--charset=ascii")
        .arg("--all-targets")
        .arg("--all-features")
        .args(extra_args)
        .current_dir(cx.crate_dir(test_name))
        .output()
        .expect("failed to run `cargo-geiger`");
    (output, cx)
}

fn make_source(cx: &Context, name: &str) -> Source {
    Source::Path(Url::from_file_path(cx.crate_dir(name)).unwrap())
}

fn make_workspace_source(cx: &Context, workspace: &str, name: &str) -> Source {
    Source::Path(
        Url::from_file_path(cx.workspace_crate_dir(workspace, name)).unwrap(),
    )
}

struct Context {
    _dir: TempDir,
    path: PathBuf,
}

impl Context {
    fn new() -> Self {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let src_path = Path::new(&manifest_dir).join("../test_crates");
        let dir = TempDir::new().unwrap();
        let copy_options = fs_extra::dir::CopyOptions {
            content_only: true,
            ..Default::default()
        };
        fs_extra::dir::copy(&src_path, dir.path(), &copy_options)
            .expect("Failed to copy tests");
        let path = dir
            .path()
            .canonicalize()
            .expect("Failed to canonicalize temporary path");
        // Canonicalizing on Windows returns a UNC path (starting with `\\?\`).
        // `cargo build` (as of 1.47.0) fails to use an overriding path dependency if the manifest
        // given to `cargo build` is a UNC path. Roudtripping to URL gets rid of the UNC prefix.
        let path = if cfg!(windows) {
            Url::from_file_path(path)
                .expect("URL from path must succeed")
                .to_file_path()
                .expect("Roundtripping path to URL must succeed")
        } else {
            path
        };
        let _dir = dir;
        Context { _dir, path }
    }

    fn crate_dir(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn workspace_crate_dir(&self, workspace: &str, name: &str) -> PathBuf {
        let mut p = self.path.clone();
        p.extend(&[workspace, name]);
        p
    }
}

fn make_package_id(cx: &Context, name: &str) -> PackageId {
    PackageId {
        name: name.into(),
        version: Version::new(0, 1, 0),
        source: make_source(cx, name),
    }
}

fn report_entry_list_to_map<I>(entries: I) -> HashMap<PackageId, ReportEntry>
where
    I: IntoIterator<Item = ReportEntry>,
{
    entries
        .into_iter()
        .map(|e| (e.package.id.clone(), e))
        .collect()
}

fn to_set<I>(items: I) -> HashSet<I::Item>
where
    I: IntoIterator,
    I::Item: Hash + Eq,
{
    items.into_iter().collect()
}

// This function does not handle all merges but works well enough to avoid repetition in these
// tests.
fn merge_test_reports(report: &mut SafetyReport, other: SafetyReport) {
    report.packages.extend(other.packages);
    report
        .packages_without_metrics
        .extend(other.packages_without_metrics);
    report
        .used_but_not_scanned_files
        .extend(other.used_but_not_scanned_files);
}

fn to_quick_report(report: SafetyReport) -> QuickSafetyReport {
    let entries = report
        .packages
        .into_iter()
        .map(|(id, entry)| {
            let quick_entry = QuickReportEntry {
                package: entry.package,
                forbids_unsafe: entry.unsafety.forbids_unsafe,
            };
            (id, quick_entry)
        })
        .collect();
    QuickSafetyReport {
        packages: entries,
        packages_without_metrics: report.packages_without_metrics,
    }
}

fn single_entry_safety_report(entry: ReportEntry) -> SafetyReport {
    SafetyReport {
        packages: report_entry_list_to_map(vec![entry]),
        ..Default::default()
    }
}

mod external {
    use super::{
        merge_test_reports, single_entry_safety_report, to_set, Context, Test,
    };
    use cargo_geiger_serde::{
        Count, CounterBlock, PackageId, PackageInfo, ReportEntry, SafetyReport,
        Source, UnsafeInfo,
    };
    use semver::Version;
    use url::Url;

    fn crates_io_source() -> Source {
        Source::Registry {
            name: "crates.io".into(),
            url: Url::parse("https://github.com/rust-lang/crates.io-index")
                .unwrap(),
        }
    }

    pub(super) fn ref_slice_package_id() -> PackageId {
        PackageId {
            name: "ref_slice".into(),
            version: Version::new(1, 1, 1),
            source: crates_io_source(),
        }
    }

    pub(super) fn ref_slice_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo::new(ref_slice_package_id()),
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 4,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 10,
                        unsafe_: 2,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        single_entry_safety_report(entry)
    }

    pub(super) fn either_package_id() -> PackageId {
        PackageId {
            name: "either".into(),
            version: Version::new(1, 5, 2),
            source: crates_io_source(),
        }
    }

    pub(super) fn either_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo::new(either_package_id()),
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 6,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 102,
                        unsafe_: 0,
                    },
                    item_impls: Count {
                        safe: 21,
                        unsafe_: 0,
                    },
                    methods: Count {
                        safe: 50,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        single_entry_safety_report(entry)
    }

    pub(super) fn doc_comment_package_id() -> PackageId {
        PackageId {
            name: "doc-comment".into(),
            version: Version::new(0, 3, 1),
            source: crates_io_source(),
        }
    }

    pub(super) fn doc_comment_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo::new(doc_comment_package_id()),
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 37,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        single_entry_safety_report(entry)
    }

    pub(super) fn itertools_package_id() -> PackageId {
        PackageId {
            name: "itertools".into(),
            version: Version::new(0, 8, 0),
            source: Source::Git {
                url: Url::parse(
                    "https://github.com/rust-itertools/itertools.git",
                )
                .unwrap(),
                rev: "8761fbefb3b209".into(),
            },
        }
    }

    pub(super) fn itertools_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![either_package_id()]),
                ..PackageInfo::new(itertools_package_id())
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 79,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 2413,
                        unsafe_: 0,
                    },
                    item_impls: Count {
                        safe: 129,
                        unsafe_: 0,
                    },
                    item_traits: Count {
                        safe: 7,
                        unsafe_: 0,
                    },
                    methods: Count {
                        safe: 180,
                        unsafe_: 0,
                    },
                },
                unused: CounterBlock {
                    functions: Count {
                        safe: 67,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 1210,
                        unsafe_: 72,
                    },
                    item_impls: Count {
                        safe: 24,
                        unsafe_: 3,
                    },
                    item_traits: Count {
                        safe: 2,
                        unsafe_: 1,
                    },
                    methods: Count {
                        safe: 29,
                        unsafe_: 3,
                    },
                },
                ..Default::default()
            },
        };
        let mut report = single_entry_safety_report(entry);
        merge_test_reports(&mut report, either_safety_report());
        report
    }

    pub(super) fn cfg_if_package_id() -> PackageId {
        PackageId {
            name: "cfg-if".into(),
            version: Version::new(0, 1, 9),
            source: crates_io_source(),
        }
    }

    pub(super) fn cfg_if_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo::new(cfg_if_package_id()),
            unsafety: Default::default(),
        };
        single_entry_safety_report(entry)
    }

    pub(super) fn generational_arena_package_id() -> PackageId {
        PackageId {
            name: "generational-arena".into(),
            version: Version::new(0, 2, 2),
            source: crates_io_source(),
        }
    }

    pub(super) fn generational_arena_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![cfg_if_package_id()]),
                ..PackageInfo::new(generational_arena_package_id())
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    exprs: Count {
                        safe: 372,
                        unsafe_: 0,
                    },
                    item_impls: Count {
                        safe: 21,
                        unsafe_: 0,
                    },
                    methods: Count {
                        safe: 39,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                unused: CounterBlock {
                    functions: Count {
                        safe: 6,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 243,
                        unsafe_: 0,
                    },
                    item_impls: Count {
                        safe: 7,
                        unsafe_: 0,
                    },
                    methods: Count {
                        safe: 8,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                forbids_unsafe: true,
            },
        };
        let mut report = single_entry_safety_report(entry);
        merge_test_reports(&mut report, cfg_if_safety_report());
        report
    }

    pub(super) fn idna_package_id() -> PackageId {
        PackageId {
            name: "idna".into(),
            version: Version::new(0, 1, 5),
            source: crates_io_source(),
        }
    }

    pub(super) fn idna_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![
                    matches_package_id(),
                    unicode_bidi_package_id(),
                    unicode_normalization_package_id(),
                ]),
                ..PackageInfo::new(idna_package_id())
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 17,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 13596,
                        unsafe_: 1,
                    },
                    ..Default::default()
                },
                unused: CounterBlock {
                    functions: Count {
                        safe: 7,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 185,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        let mut report = single_entry_safety_report(entry);
        merge_test_reports(&mut report, matches_safety_report());
        merge_test_reports(&mut report, unicode_bidi_safety_report());
        merge_test_reports(&mut report, unicode_normalization_safety_report());
        report
    }

    pub(super) fn matches_package_id() -> PackageId {
        PackageId {
            name: "matches".into(),
            version: Version::new(0, 1, 8),
            source: crates_io_source(),
        }
    }

    pub(super) fn matches_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo::new(matches_package_id()),
            unsafety: Default::default(),
        };
        single_entry_safety_report(entry)
    }

    pub(super) fn smallvec_package_id() -> PackageId {
        PackageId {
            name: "smallvec".into(),
            version: Version::new(0, 6, 9),
            source: crates_io_source(),
        }
    }

    pub(super) fn smallvec_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo::new(smallvec_package_id()),
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 0,
                        unsafe_: 2,
                    },
                    exprs: Count {
                        safe: 291,
                        unsafe_: 354,
                    },
                    item_impls: Count {
                        safe: 48,
                        unsafe_: 4,
                    },
                    item_traits: Count {
                        safe: 3,
                        unsafe_: 1,
                    },
                    methods: Count {
                        safe: 92,
                        unsafe_: 13,
                    },
                },
                unused: CounterBlock {
                    functions: Count {
                        safe: 18,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 126,
                        unsafe_: 0,
                    },
                    item_impls: Count {
                        safe: 2,
                        unsafe_: 0,
                    },
                    item_traits: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    methods: Count {
                        safe: 14,
                        unsafe_: 0,
                    },
                },
                ..Default::default()
            },
        };
        single_entry_safety_report(entry)
    }

    pub(super) fn unicode_bidi_package_id() -> PackageId {
        PackageId {
            name: "unicode-bidi".into(),
            version: Version::new(0, 3, 4),
            source: crates_io_source(),
        }
    }

    pub(super) fn unicode_bidi_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![matches_package_id()]),
                ..PackageInfo::new(unicode_bidi_package_id())
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 16,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 2119,
                        unsafe_: 0,
                    },
                    item_impls: Count {
                        safe: 8,
                        unsafe_: 0,
                    },
                    methods: Count {
                        safe: 31,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                forbids_unsafe: true,
                ..Default::default()
            },
        };
        let mut report = single_entry_safety_report(entry);
        merge_test_reports(&mut report, matches_safety_report());
        report
    }

    pub(super) fn unicode_normalization_package_id() -> PackageId {
        PackageId {
            name: "unicode-normalization".into(),
            version: Version::new(0, 1, 8),
            source: crates_io_source(),
        }
    }

    pub(super) fn unicode_normalization_safety_report() -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![smallvec_package_id()]),
                ..PackageInfo::new(unicode_normalization_package_id())
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 37,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 12901,
                        unsafe_: 20,
                    },
                    item_impls: Count {
                        safe: 9,
                        unsafe_: 0,
                    },
                    item_traits: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    methods: Count {
                        safe: 21,
                        unsafe_: 0,
                    },
                },
                unused: CounterBlock {
                    functions: Count {
                        safe: 22,
                        unsafe_: 0,
                    },
                    exprs: Count {
                        safe: 84,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        let mut report = single_entry_safety_report(entry);
        merge_test_reports(&mut report, smallvec_safety_report());
        report
    }

    pub(super) fn num_cpus_package_id(cx: &Context) -> PackageId {
        PackageId {
            name: "num_cpus".into(),
            version: Version::new(1, 10, 1),
            source: super::make_workspace_source(cx, "support", "num_cpus"),
        }
    }

    pub(super) fn num_cpus_safety_report(cx: &Context) -> SafetyReport {
        let entry = ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![super::make_package_id(
                    cx,
                    super::Test1::NAME,
                )]),
                ..PackageInfo::new(num_cpus_package_id(cx))
            },
            unsafety: UnsafeInfo {
                used: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 0,
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        let mut report = single_entry_safety_report(entry);
        merge_test_reports(&mut report, super::Test1.expected_report(cx));
        report
    }
}
