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
    use rstest::*;
    use std::io::Write;
    use tempfile::tempdir;

    const FILE_CONTENT_STRING: &str = "use std::io::Write;

pub unsafe fn f() {
    unimplemented!()
}

pub fn g() {
    std::io::stdout().write_all(unsafe {
        std::str::from_utf8_unchecked(b\"binarystring\")
    }.as_bytes()).unwrap();
}

#[no_mangle]
pub fn h() {
    unimplemented!()
}

#[export_name = \"exported_g\"]
pub fn g() {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        unsafe {
            println!(\"Inside unsafe\");
        }
    }
}
";

    #[rstest(
        input_include_tests,
        expected_rs_file_metrics,
        case(
        IncludeTests::Yes,
        RsFileMetrics {
            counters: CounterBlock {
                functions: Count {
                    safe: 2,
                    unsafe_: 3
                },
                exprs: Count {
                    safe: 4,
                    unsafe_: 5
                },
                item_impls: Count {
                    safe: 0,
                    unsafe_: 0
                },
                item_traits: Count {
                    safe: 0,
                    unsafe_: 0
                },
                methods: Count {
                    safe: 0,
                    unsafe_: 0
                }
            },
            forbids_unsafe: false
        }
        ),
        case(
            IncludeTests::No,
            RsFileMetrics {
                counters: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 3
                    },
                    exprs: Count {
                        safe: 4,
                        unsafe_: 4
                    },
                    item_impls: Count {
                        safe: 0,
                        unsafe_: 0
                    },
                    item_traits: Count {
                        safe: 0,
                        unsafe_: 0
                    },
                    methods: Count {
                        safe: 0,
                        unsafe_: 0
                    }
                },
                forbids_unsafe: false
            }
        )
    )]
    fn find_unsafe_in_file_test_no_errors(
        input_include_tests: IncludeTests,
        expected_rs_file_metrics: RsFileMetrics,
    ) {
        if let Ok(temp_dir) = tempdir() {
            let lib_file_path = temp_dir.path().join("lib.rs");
            let mut file = File::create(lib_file_path.clone()).unwrap();

            writeln!(file, "{}", FILE_CONTENT_STRING).unwrap();

            let unsafe_in_file_result =
                find_unsafe_in_file(&lib_file_path, input_include_tests);

            assert!(unsafe_in_file_result.is_ok());

            let unsafe_in_file = unsafe_in_file_result.unwrap();

            assert_eq!(unsafe_in_file, expected_rs_file_metrics);
        }
    }

    #[rstest(
        input_include_tests,
        expected_rs_file_metrics,
        case(
            IncludeTests::Yes,
            RsFileMetrics {
                counters: CounterBlock {
                    functions: Count {
                        safe: 2,
                        unsafe_: 3
                    },
                    exprs: Count {
                        safe: 4,
                        unsafe_: 5
                    },
                    item_impls: Count {
                        safe: 0,
                        unsafe_: 0
                    },
                    item_traits: Count {
                        safe: 0,
                        unsafe_: 0
                    },
                    methods: Count {
                        safe: 0,
                        unsafe_: 0
                    }
                },
                forbids_unsafe: false
            }
        ),
        case(
            IncludeTests::No,
            RsFileMetrics {
                counters: CounterBlock {
                    functions: Count {
                        safe: 1,
                        unsafe_: 3
                    },
                    exprs: Count {
                        safe: 4,
                        unsafe_: 4
                    },
                    item_impls: Count {
                        safe: 0,
                        unsafe_: 0
                    },
                    item_traits: Count {
                        safe: 0,
                        unsafe_: 0
                    },
                    methods: Count {
                        safe: 0,
                        unsafe_: 0
                    }
                },
                forbids_unsafe: false
            }
        )
    )]
    fn find_unsafe_in_string_test(
        input_include_tests: IncludeTests,
        expected_rs_file_metrics: RsFileMetrics,
    ) {
        let unsafe_in_string_result =
            find_unsafe_in_string(FILE_CONTENT_STRING, input_include_tests);

        assert!(unsafe_in_string_result.is_ok());
        let unsafe_in_string = unsafe_in_string_result.unwrap();

        assert_eq!(unsafe_in_string, expected_rs_file_metrics);
    }
}
