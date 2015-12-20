extern crate cargo;
extern crate docopt;
extern crate rustc_serialize;

use cargo::{Config, CliResult};
use cargo::core::Source;
use cargo::core::registry::PackageRegistry;
use cargo::ops;
use cargo::util::important_paths;
use cargo::sources::path::PathSource;

static USAGE: &'static str = "
Display a tree visualization of a dependency graph

Usage: cargo tree [options]
       cargo tree --help

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to analyze
    -v, --verbose           Use verbose output
";

#[derive(RustcDecodable)]
struct Flags {
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
}

fn main() {
    cargo::execute_main_without_stdin(real_main, false, USAGE);
}

fn real_main(flags: Flags, config: &Config) -> CliResult<Option<()>> {
    try!(config.shell().set_verbosity(flags.flag_verbose, false));

    // Load the root package
    let root = try!(important_paths::find_root_manifest_for_cwd(flags.flag_manifest_path));
    let mut source = try!(PathSource::for_path(root.parent().unwrap(), config));
    try!(source.update());
    let package = try!(source.root_package());

    // Resolve all dependencies (generating or using Cargo.lock if necessary)
    let mut registry = PackageRegistry::new(config);
    try!(registry.add_sources(&[package.package_id().source_id().clone()]));
    let resolve = try!(ops::resolve_pkg(&mut registry, &package));
    
    println!("{:#?}", resolve);

    Ok(None)
}
