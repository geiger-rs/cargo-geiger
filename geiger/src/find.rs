use super::{IncludeTests, RsFileMetrics, ScanFileError};

use crate::geiger_syn_visitor::GeigerSynVisitor;

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Scan a single file for `unsafe` usage.
pub fn find_unsafe_in_file(
    path: &Path,
    include_tests: IncludeTests,
) -> Result<RsFileMetrics, ScanFileError> {
    let mut file = File::open(path)
        .map_err(|e| ScanFileError::Io(e, path.to_path_buf()))?;
    let mut src = vec![];
    file.read_to_end(&mut src)
        .map_err(|e| ScanFileError::Io(e, path.to_path_buf()))?;
    let src = String::from_utf8(src)
        .map_err(|e| ScanFileError::Utf8(e, path.to_path_buf()))?;
    find_unsafe_in_string(&src, include_tests)
        .map_err(|e| ScanFileError::Syn(e, path.to_path_buf()))
}

pub fn find_unsafe_in_string(
    src: &str,
    include_tests: IncludeTests,
) -> Result<RsFileMetrics, syn::Error> {
    use syn::visit::Visit;
    let syntax = syn::parse_file(src)?;
    let mut vis = GeigerSynVisitor::new(include_tests);
    vis.visit_file(&syntax);
    Ok(vis.metrics)
}

#[cfg(test)]
mod find_tests {
    use super::*;

    use cargo_geiger_serde::{Count, CounterBlock};
    use tempfile::tempdir;

    const DEFAULT_COUNTERS: CounterBlock = CounterBlock {
        functions: Count {
            safe: 0,
            unsafe_: 0,
        },
        exprs: Count {
            safe: 0,
            unsafe_: 0,
        },
        item_impls: Count {
            safe: 0,
            unsafe_: 0,
        },
        item_traits: Count {
            safe: 0,
            unsafe_: 0,
        },
        methods: Count {
            safe: 0,
            unsafe_: 0,
        },
    };
    const DEFAULT_METRICS: RsFileMetrics = RsFileMetrics {
        counters: DEFAULT_COUNTERS,
        forbids_unsafe: false,
    };

    const FILE_CONTENT_STRING: &str = "use std::io::Write;

pub unsafe fn f() {
    f();
}

pub fn g() {
    std::io::stdout().write_all(unsafe {
        std::str::from_utf8_unchecked(b\"binarystring\")
    }.as_bytes()).unwrap();
}

#[no_mangle]
pub fn h() {
    f();
}

#[export_name = \"exported_g\"]
pub fn g() {
    f();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        unsafe {
            f();
        }
    }
}
";

    #[test]
    fn find_unsafe() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("lib.rs");
        std::fs::write(&file_path, FILE_CONTENT_STRING).unwrap();

        let from_file =
            find_unsafe_in_file(&file_path, IncludeTests::No).unwrap();
        let from_string =
            find_unsafe_in_string(FILE_CONTENT_STRING, IncludeTests::No)
                .unwrap();
        let expected = RsFileMetrics {
            counters: CounterBlock {
                functions: Count {
                    safe: 1,
                    unsafe_: 3,
                },
                exprs: Count {
                    safe: 4,
                    unsafe_: 4,
                },
                ..DEFAULT_COUNTERS
            },
            ..DEFAULT_METRICS
        };
        assert_eq!(from_file, expected);
        assert_eq!(from_string, expected);

        let from_file =
            find_unsafe_in_file(&file_path, IncludeTests::Yes).unwrap();
        let from_string =
            find_unsafe_in_string(FILE_CONTENT_STRING, IncludeTests::Yes)
                .unwrap();
        let expected = RsFileMetrics {
            counters: CounterBlock {
                functions: Count {
                    safe: 2,
                    unsafe_: 3,
                },
                exprs: Count {
                    safe: 4,
                    unsafe_: 5,
                },
                ..DEFAULT_COUNTERS
            },
            ..DEFAULT_METRICS
        };
        assert_eq!(from_file, expected);
        assert_eq!(from_string, expected);
    }

    #[test]
    fn forbids_unsafe() {
        let expected = RsFileMetrics {
            forbids_unsafe: true,
            ..DEFAULT_METRICS
        };
        let actual =
            find_unsafe_in_string("#![forbid(unsafe_code)]", IncludeTests::No)
                .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn counters_functions() {
        let expected = RsFileMetrics {
            counters: CounterBlock {
                functions: Count {
                    safe: 2,
                    unsafe_: 3,
                },
                exprs: Count {
                    safe: 2,
                    unsafe_: 3,
                },
                ..DEFAULT_COUNTERS
            },
            ..DEFAULT_METRICS
        };
        let file = "
            pub fn f() { f(); }
            pub fn f() { f(); }
            pub unsafe fn f() { f(); }
            #[no_mangle]
            pub fn f() { f(); }
            #[export_name = \"exported_e\"]
            pub unsafe fn f() { f(); }
        ";
        let actual = find_unsafe_in_string(file, IncludeTests::No).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn counters_exprs() {
        let file = "
            pub fn f() {
                f();
                x.f();
                let x = *y;
                println!(\"abc\"); // The `syn` crate v2.0.60 doesn't visit macros.
                let x = 1; // Literal expressions are not counted.
            }
            pub fn f() { unsafe { let x = f(); } }
            pub unsafe fn f() { let x = f(); }
            #[cfg(test)]
            mod tests {
                pub fn f() { f(); }
            }
            #[test]
            pub fn f() { f(); }
        ";
        let expected = RsFileMetrics {
            counters: CounterBlock {
                functions: Count {
                    safe: 2,
                    unsafe_: 1,
                },
                exprs: Count {
                    safe: 3,
                    unsafe_: 2,
                },
                ..DEFAULT_COUNTERS
            },
            ..DEFAULT_METRICS
        };
        let actual = find_unsafe_in_string(file, IncludeTests::No).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn counters_exprs_include_tests() {
        let file = "
            pub fn f() { f(); }
            pub unsafe fn f() { f(); }
            #[cfg(test)]
            mod tests {
                pub unsafe fn f() { f(); }
                pub fn f() {
                    f();
                    unsafe { f(); }
                }
            }
            #[test]
            pub fn f() {
                f();
                unsafe { f(); }
            }
        ";
        let expected = RsFileMetrics {
            counters: CounterBlock {
                functions: Count {
                    safe: 3,
                    unsafe_: 2,
                },
                exprs: Count {
                    safe: 3,
                    unsafe_: 4,
                },
                ..DEFAULT_COUNTERS
            },
            ..DEFAULT_METRICS
        };
        let actual = find_unsafe_in_string(file, IncludeTests::Yes).unwrap();
        assert_eq!(actual, expected);
    }
}
