#![forbid(unsafe_code)]
#![forbid(warnings)]

use assert_cmd::prelude::*;
use insta::assert_snapshot;
use rstest::rstest_parametrize;

use std::env;
use std::path::Path;
use std::process::Command;

#[rstest_parametrize(
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

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let test_case_root_dir =
        Path::new(&manifest_dir).join("../test_crates").join(name);

    let result = Command::cargo_bin("cargo-geiger")
        .unwrap()
        .arg("geiger")
        .arg("--color=never")
        .arg("--quiet=true")
        .arg("--charset=ascii")
        .arg("--all-targets")
        .arg("--all-features")
        .current_dir(test_case_root_dir)
        .output()
        .expect("failed to run `cargo-geiger`");

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
