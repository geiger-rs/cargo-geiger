use crate::context::Context;

use assert_cmd::prelude::*;
use std::process::{Command, Output};

pub fn run_geiger_with<I>(test_name: &str, extra_args: I) -> (Output, Context)
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
        .arg("--output-format=Ascii")
        .arg("--all-targets")
        .arg("--all-features")
        .args(extra_args)
        .current_dir(cx.crate_dir(test_name))
        .output()
        .expect("failed to run `cargo-geiger`");
    (output, cx)
}
