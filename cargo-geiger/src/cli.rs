//! This module provides the bulk of the code for the `cargo-geiger` executable.

// TODO: Review the module structure in this crate. There is very tight coupling
// between the main.rs and this module. Should this module be split into smaller
// parts? The printing and scanning can probably be further decoupled to provide
// a better base for adding more output formats.

// TODO: Investigate how cargo-clippy is implemented. Is it using syn?  Is is
// using rustc? Is it implementing a compiler plugin?

// TODO: Consider making this a lib.rs (again) and expose a full API, excluding
// only the terminal output..? That API would be dependent on cargo.

use cargo::core::package::PackageSet;
use cargo::core::registry::PackageRegistry;
use cargo::core::resolver::ResolveOpts;
use cargo::core::{Package, PackageId, PackageIdSpec, Resolve, Workspace};
use cargo::ops;
use cargo::util::{self, important_paths, CargoResult};
use cargo::Config;
use cargo_platform::Cfg;
use std::path::PathBuf;
use std::str::{self, FromStr};

/// TODO: Write proper documentation for this.
/// This function seems to be looking up the active flags for conditional
/// compilation (cargo_platform::Cfg instances).
pub fn get_cfgs(
    config: &Config,
    target: &Option<String>,
    ws: &Workspace,
) -> CargoResult<Option<Vec<Cfg>>> {
    let mut process = util::process(&config.load_global_rustc(Some(ws))?.path);
    process.arg("--print=cfg").env_remove("RUST_LOG");
    if let Some(ref s) = *target {
        process.arg("--target").arg(s);
    }
    let output = match process.exec_with_output() {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };
    let output = str::from_utf8(&output.stdout).unwrap();
    let lines = output.lines();
    Ok(Some(
        lines
            .map(|s| Cfg::from_str(s).map_err(|e| e.into()))
            .collect::<CargoResult<Vec<_>>>()?,
    ))
}

pub fn get_workspace(
    config: &Config,
    manifest_path: Option<PathBuf>,
) -> CargoResult<Workspace> {
    let root = match manifest_path {
        Some(path) => path,
        None => important_paths::find_root_manifest_for_wd(config.cwd())?,
    };
    Workspace::new(&root, config)
}

pub fn get_registry<'a>(
    config: &'a Config,
    package: &Package,
) -> CargoResult<PackageRegistry<'a>> {
    let mut registry = PackageRegistry::new(config)?;
    registry.add_sources(Some(package.package_id().source_id()))?;
    Ok(registry)
}

pub fn resolve<'a, 'cfg>(
    package_id: PackageId,
    registry: &mut PackageRegistry<'cfg>,
    ws: &'a Workspace<'cfg>,
    features: &[String],
    all_features: bool,
    no_default_features: bool,
) -> CargoResult<(PackageSet<'a>, Resolve)> {
    let dev_deps = true; // TODO: Review this.
    let uses_default_features = !no_default_features;
    let opts = ResolveOpts::new(
        dev_deps,
        features,
        all_features,
        uses_default_features,
    );
    let prev = ops::load_pkg_lockfile(ws)?;
    let resolve = ops::resolve_with_previous(
        registry,
        ws,
        &opts,
        prev.as_ref(),
        None,
        &[PackageIdSpec::from_package_id(package_id)],
        true,
    )?;
    let packages = ops::get_resolved_packages(
        &resolve,
        PackageRegistry::new(ws.config())?,
    )?;
    Ok((packages, resolve))
}

// TODO: Make a wrapper type for canonical paths and hide all mutable access.
