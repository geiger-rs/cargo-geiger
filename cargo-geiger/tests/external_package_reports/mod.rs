use crate::context::Context;
use crate::integration_test::IntegrationTest;
use crate::report::{merge_test_reports, single_entry_safety_report, to_set};
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

pub fn ref_slice_package_id() -> PackageId {
    PackageId {
        name: "ref_slice".into(),
        version: Version::new(1, 1, 1),
        source: crates_io_source(),
    }
}

pub fn ref_slice_safety_report() -> SafetyReport {
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

pub fn either_package_id() -> PackageId {
    PackageId {
        name: "either".into(),
        version: Version::new(1, 5, 2),
        source: crates_io_source(),
    }
}

pub fn either_safety_report() -> SafetyReport {
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

pub fn doc_comment_package_id() -> PackageId {
    PackageId {
        name: "doc-comment".into(),
        version: Version::new(0, 3, 1),
        source: crates_io_source(),
    }
}

pub fn doc_comment_safety_report() -> SafetyReport {
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

pub fn itertools_package_id() -> PackageId {
    PackageId {
        name: "itertools".into(),
        version: Version::new(0, 8, 0),
        source: Source::Git {
            url: Url::parse("https://github.com/rust-itertools/itertools.git")
                .unwrap(),
            rev: "8761fbefb3b209".into(),
        },
    }
}

pub fn itertools_safety_report() -> SafetyReport {
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

pub fn cfg_if_package_id() -> PackageId {
    PackageId {
        name: "cfg-if".into(),
        version: Version::new(0, 1, 9),
        source: crates_io_source(),
    }
}

pub fn cfg_if_safety_report() -> SafetyReport {
    let entry = ReportEntry {
        package: PackageInfo::new(cfg_if_package_id()),
        unsafety: Default::default(),
    };
    single_entry_safety_report(entry)
}

pub fn generational_arena_package_id() -> PackageId {
    PackageId {
        name: "generational-arena".into(),
        version: Version::new(0, 2, 2),
        source: crates_io_source(),
    }
}

pub fn generational_arena_safety_report() -> SafetyReport {
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

pub fn idna_package_id() -> PackageId {
    PackageId {
        name: "idna".into(),
        version: Version::new(0, 1, 5),
        source: crates_io_source(),
    }
}

pub fn idna_safety_report() -> SafetyReport {
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

pub fn matches_package_id() -> PackageId {
    PackageId {
        name: "matches".into(),
        version: Version::new(0, 1, 8),
        source: crates_io_source(),
    }
}

pub fn matches_safety_report() -> SafetyReport {
    let entry = ReportEntry {
        package: PackageInfo::new(matches_package_id()),
        unsafety: Default::default(),
    };
    single_entry_safety_report(entry)
}

pub fn smallvec_package_id() -> PackageId {
    PackageId {
        name: "smallvec".into(),
        version: Version::new(0, 6, 9),
        source: crates_io_source(),
    }
}

pub fn smallvec_safety_report() -> SafetyReport {
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

pub fn unicode_bidi_package_id() -> PackageId {
    PackageId {
        name: "unicode-bidi".into(),
        version: Version::new(0, 3, 4),
        source: crates_io_source(),
    }
}

pub fn unicode_bidi_safety_report() -> SafetyReport {
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

pub fn num_cpus_package_id(cx: &Context) -> PackageId {
    PackageId {
        name: "num_cpus".into(),
        version: Version::new(1, 10, 1),
        source: super::make_workspace_source(cx, "support", "num_cpus"),
    }
}

pub fn num_cpus_safety_report(cx: &Context) -> SafetyReport {
    let entry = ReportEntry {
        package: PackageInfo {
            dependencies: to_set(vec![make_package_id(cx, super::Test1::NAME)]),
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

pub fn make_package_id(cx: &Context, name: &str) -> PackageId {
    PackageId {
        name: name.into(),
        version: Version::new(0, 1, 0),
        source: make_source(cx, name),
    }
}

fn make_source(cx: &Context, name: &str) -> Source {
    Source::Path(Url::from_file_path(cx.crate_dir(name)).unwrap())
}
