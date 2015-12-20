extern crate cargo;
extern crate docopt;
extern crate rustc_serialize;

use cargo::{Config, CliResult};
use cargo::core::{Source, PackageId, Resolve};
use cargo::core::registry::PackageRegistry;
use cargo::ops;
use cargo::util::important_paths;
use cargo::sources::path::PathSource;
use std::collections::HashSet;

#[cfg_attr(rustfmt, rustfmt_skip)]
static USAGE: &'static str = "
Display a tree visualization of a dependency graph

Usage: cargo tree [options]
       cargo tree --help

Options:
    -h, --help              Print this message
    --charset CHARSET       Set the character set to use in output. Valid
                            values: UTF8, ASCII [default: UTF8]
    --manifest-path PATH    Path to the manifest to analyze
    -v, --verbose           Use verbose output
    -q, --quiet             No output printed to stdout other than the tree
";

#[derive(RustcDecodable)]
struct Flags {
    flag_charset: Charset,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_quiet: bool,
}

#[derive(RustcDecodable)]
enum Charset {
    Utf8,
    Ascii,
}

struct Symbols {
    down: &'static str,
    tee: &'static str,
    ell: &'static str,
    right: &'static str,
}

static UTF8_SYMBOLS: Symbols = Symbols {
    down: "│",
    tee: "├",
    ell: "└",
    right: "─",
};

static ASCII_SYMBOLS: Symbols = Symbols {
    down: "|",
    tee: "|",
    ell: "`",
    right: "-",
};

fn main() {
    cargo::execute_main_without_stdin(real_main, false, USAGE);
}

fn real_main(flags: Flags, config: &Config) -> CliResult<Option<()>> {
    try!(config.shell().set_verbosity(flags.flag_verbose, flags.flag_quiet));

    let symbols = match flags.flag_charset {
        Charset::Ascii => &ASCII_SYMBOLS,
        Charset::Utf8 => &UTF8_SYMBOLS,
    };

    // Load the root package
    let root = try!(important_paths::find_root_manifest_for_cwd(flags.flag_manifest_path));
    let mut source = try!(PathSource::for_path(root.parent().unwrap(), config));
    try!(source.update());
    let package = try!(source.root_package());

    // Resolve all dependencies (generating or using Cargo.lock if necessary)
    let mut registry = PackageRegistry::new(config);
    try!(registry.add_sources(&[package.package_id().source_id().clone()]));
    let resolve = try!(ops::resolve_pkg(&mut registry, &package));

    print_tree(&resolve, symbols);

    Ok(None)
}

fn print_tree(resolve: &Resolve, symbols: &Symbols) {
    let mut visited_deps = HashSet::new();
    let mut levels_continue = vec![];

    print_dependency(resolve.root(),
                     resolve,
                     symbols,
                     &mut visited_deps,
                     &mut levels_continue);
}

fn print_dependency<'a>(package: &'a PackageId,
                        resolve: &'a Resolve,
                        symbols: &Symbols,
                        visited_deps: &mut HashSet<&'a PackageId>,
                        levels_continue: &mut Vec<bool>) {
    if let Some((&last_continues, rest)) = levels_continue.split_last() {
        for &continues in rest {
            let c = if continues {
                symbols.down
            } else {
                " "
            };
            print!("{}  ", c);
        }

        let c = if last_continues {
            symbols.tee
        } else {
            symbols.ell
        };
        print!("{0}{1}{1} ", c, symbols.right);
    }

    let new = visited_deps.insert(package);
    let star = if new {
        ""
    } else {
        " (*)"
    };

    println!("{}{}", package, star);

    if !new {
        return;
    }

    // Resolve uses Hash data types internally but we want consistent output
    let mut deps = resolve.deps(package).unwrap().collect::<Vec<_>>();
    deps.sort();
    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        print_dependency(dependency, resolve, symbols, visited_deps, levels_continue);
        levels_continue.pop();
    }
}
