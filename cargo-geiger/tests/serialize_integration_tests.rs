#![forbid(unsafe_code)]
#![forbid(warnings)]

mod context;
mod external_package_reports;
mod integration_test;
mod report;
mod run;

use self::context::Context;
use self::external_package_reports::make_package_id;
use self::integration_test::IntegrationTest;
use self::report::{merge_test_reports, single_entry_safety_report, to_set};

use cargo_geiger_serde::{
    Count, CounterBlock, PackageInfo, ReportEntry, SafetyReport, Source,
    UnsafeInfo,
};
use rstest::rstest;
use std::path::PathBuf;
use url::Url;

#[rstest]
fn serialize_test1_report() {
    Test1.run();
}

#[rstest]
fn serialize_test2_report() {
    Test2.run();
}

#[rstest]
fn serialize_test3_report() {
    Test3.run();
}

#[rstest]
fn serialize_test4_report() {
    Test4.run();
}

#[rstest]
fn serialize_test6_report() {
    Test6.run();
}

#[rstest]
fn serialize_test7_report() {
    Test7.run();
}

#[rstest]
fn serialize_test1_quick_report() {
    Test1.run_quick();
}

#[rstest]
fn serialize_test2_quick_report() {
    Test2.run_quick();
}

#[rstest]
fn serialize_test3_quick_report() {
    Test3.run_quick();
}

#[rstest]
fn serialize_test4_quick_report() {
    Test4.run_quick();
}

#[rstest]
fn serialize_test6_quick_report() {
    Test6.run_quick();
}

#[rstest]
fn serialize_test7_quick_report() {
    Test7.run_quick();
}

struct Test1;

impl IntegrationTest for Test1 {
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

impl IntegrationTest for Test2 {
    const NAME: &'static str = "test2_package_with_shallow_deps";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(&mut report, Test1.expected_report(cx));
        merge_test_reports(
            &mut report,
            external_package_reports::ref_slice_safety_report(),
        );
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![
                    make_package_id(cx, Test1::NAME),
                    external_package_reports::ref_slice_package_id(),
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

impl IntegrationTest for Test3 {
    const NAME: &'static str = "test3_package_with_nested_deps";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(
            &mut report,
            external_package_reports::itertools_safety_report(),
        );
        merge_test_reports(
            &mut report,
            external_package_reports::doc_comment_safety_report(),
        );
        merge_test_reports(&mut report, Test2.expected_report(cx));
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![
                    make_package_id(cx, Test2::NAME),
                    external_package_reports::itertools_package_id(),
                    external_package_reports::doc_comment_package_id(),
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

impl IntegrationTest for Test4 {
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

impl IntegrationTest for Test6 {
    const NAME: &'static str = "test6_cargo_lock_out_of_date";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(
            &mut report,
            external_package_reports::generational_arena_safety_report(),
        );
        merge_test_reports(
            &mut report,
            external_package_reports::idna_safety_report(),
        );
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![
                    external_package_reports::generational_arena_package_id(),
                    external_package_reports::idna_package_id(),
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

impl IntegrationTest for Test7 {
    const NAME: &'static str = "test7_package_with_patched_dep";

    fn expected_report(&self, cx: &Context) -> SafetyReport {
        let mut report =
            single_entry_safety_report(self.expected_report_entry(cx));
        merge_test_reports(
            &mut report,
            external_package_reports::num_cpus_safety_report(cx),
        );
        report
    }

    fn expected_report_entry(&self, cx: &Context) -> ReportEntry {
        ReportEntry {
            package: PackageInfo {
                dependencies: to_set(vec![
                    external_package_reports::num_cpus_package_id(cx),
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

fn make_workspace_source(cx: &Context, workspace: &str, name: &str) -> Source {
    Source::Path(
        Url::from_file_path(cx.workspace_crate_dir(workspace, name)).unwrap(),
    )
}

trait WorkspaceCrateDir {
    fn workspace_crate_dir(&self, workspace: &str, name: &str) -> PathBuf;
}

impl WorkspaceCrateDir for Context {
    fn workspace_crate_dir(&self, workspace: &str, name: &str) -> PathBuf {
        let mut p = self.path.clone();
        p.extend(&[workspace, name]);
        p
    }
}
