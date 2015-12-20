extern crate cargo;
extern crate docopt;
extern crate rustc_serialize;

use std::env;
use cargo::{Config, CliResult};

static USAGE: &'static str = "
Display a tree visualization of a dependency graph

Usage: cargo tree [options]
       cargo tree --help

Options:
    -h, --help          Print this message
    --lock-path PATH    Path to the Cargo.lock file to analyze
";

#[derive(RustcDecodable)]
struct Flags {
    flag_help: bool,
    flag_lock_path: String,
}

fn main() {
    cargo::execute_main_without_stdin(real_main, false, USAGE);
}

fn real_main(flags: Flags, config: &Config) -> CliResult<Option<()>> {
    Ok(None)
}
