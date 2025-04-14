// TODO: Review the module structure in this crate. There is very tight coupling
// between the main.rs and this module. Should this module be split into smaller
// parts? The printing and scanning can probably be further decoupled to provide
// a better base for adding more output formats.

// TODO: Investigate how cargo-clippy is implemented. Is it using syn?  Is is
// using rustc? Is it implementing a compiler plugin?

use crate::args::Args;

// TODO: Consider making this a lib.rs (again) and expose a full API, excluding
// only the terminal output..? That API would be dependent on cargo.
use cargo::core::Workspace;
use cargo::util::{important_paths, CargoResult};
use cargo::GlobalContext;
use cargo_platform::Cfg;
use krates::cm::{CargoOpt, MetadataCommand};
use krates::Builder as KratesBuilder;
use krates::Krates;
use std::path::PathBuf;
use std::process::Command;
use std::str::{self, FromStr};

pub fn get_cargo_metadata(
    args: &Args,
    config: &GlobalContext,
) -> CargoResult<krates::cm::Metadata> {
    let root_manifest_path = match args.manifest_path.clone() {
        Some(path) => path,
        None => important_paths::find_root_manifest_for_wd(config.cwd())?,
    };

    let mut metadata_command = MetadataCommand::new();
    metadata_command.manifest_path(root_manifest_path);
    if !args.target_args.all_targets {
        if let Some(specified_target) = args
            .target_args
            .target
            .as_deref()
            .map(|s| s.to_string())
            .or_else(get_host_target)
        {
            metadata_command.other_options([
                "--filter-platform".to_string(),
                specified_target,
            ]);
        }
    }

    if let Some(metadata_command_features) = match &args.features_args {
        features_args if features_args.all_features => {
            Some(CargoOpt::AllFeatures)
        }
        features_args if features_args.no_default_features => {
            Some(CargoOpt::NoDefaultFeatures)
        }
        features_args if !features_args.features.is_empty() => {
            Some(CargoOpt::SomeFeatures(args.features_args.features.clone()))
        }
        _ => None,
    } {
        metadata_command.features(metadata_command_features);
    }

    Ok(metadata_command.exec()?)
}

/// TODO: Write proper documentation for this.
/// This function seems to be looking up the active flags for conditional
/// compilation (`cargo_platform::Cfg` instances).
pub fn get_cfgs(
    global_rustc_path: &PathBuf,
    target: &Option<String>,
) -> CargoResult<Option<Vec<Cfg>>> {
    let mut process = cargo_util::ProcessBuilder::new(global_rustc_path);
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

pub fn get_krates(
    cargo_metadata: &krates::cm::Metadata,
) -> CargoResult<Krates> {
    Ok(KratesBuilder::new()
        .build_with_metadata(cargo_metadata.clone(), |_| ())?)
}

pub fn get_workspace(
    gctx: &GlobalContext,
    manifest_path: Option<PathBuf>,
) -> CargoResult<Workspace> {
    let root = match manifest_path {
        Some(path) => path,
        None => important_paths::find_root_manifest_for_wd(gctx.cwd())?,
    };
    Workspace::new(&root, gctx)
}

/// Query the installed rust compiler for the default host triple
fn get_host_target() -> Option<String> {
    let rustc_output = Command::new("rustc").arg("-vV").output().ok()?;
    if rustc_output.status.success() {
        let output_str = String::from_utf8(rustc_output.stdout).ok()?;
        for line in output_str.lines() {
            if line.starts_with("host:") {
                let parts: Vec<_> = line.split(" ").collect();
                return parts.get(1).map(|s| s.to_string());
            }
        }
        None
    } else {
        None
    }
}

// TODO: Make a wrapper type for canonical paths and hide all mutable access.

#[cfg(test)]
mod cli_tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn get_cargo_metadata_test() {
        let args = Args::default();
        let gctx = GlobalContext::default().unwrap();

        let cargo_metadata_result = get_cargo_metadata(&args, &gctx);

        assert!(cargo_metadata_result.is_ok());
    }

    #[rstest]
    fn get_cfgs_test() {
        let gctx = GlobalContext::default().unwrap();
        let target: Option<String> = None;
        let root =
            important_paths::find_root_manifest_for_wd(gctx.cwd()).unwrap();
        let workspace = Workspace::new(&root, &gctx).unwrap();

        let global_rustc = gctx.load_global_rustc(Some(&workspace)).unwrap();

        let cfgs = get_cfgs(&global_rustc.path, &target);

        assert!(cfgs.is_ok());
        let cfg_vec_option = cfgs.unwrap();
        assert!(cfg_vec_option.is_some());
        let cfg_vec = cfg_vec_option.unwrap();

        let mut names =
            cfg_vec.iter().filter(|cfg| matches!(cfg, Cfg::Name(_)));

        let mut key_pairs = cfg_vec
            .iter()
            .filter(|cfg| matches!(cfg, Cfg::KeyPair(_, _)));

        assert!(names.next().is_some());
        assert!(key_pairs.next().is_some());
    }

    #[rstest]
    fn get_krates_test() {
        let args = Args::default();
        let gctx = GlobalContext::default().unwrap();
        let cargo_metadata = get_cargo_metadata(&args, &gctx).unwrap();

        let krates_result = get_krates(&cargo_metadata);
        assert!(krates_result.is_ok());
    }

    #[rstest]
    fn get_workspace_test() {
        let gctx = GlobalContext::default().unwrap();
        let manifest_path: Option<PathBuf> = None;

        let workspace_cargo_result = get_workspace(&gctx, manifest_path);
        assert!(workspace_cargo_result.is_ok());
        let workspace = workspace_cargo_result.unwrap();

        let package_result = workspace.current();
        assert!(package_result.is_ok());
        let package = package_result.unwrap();

        assert_eq!(package.package_id().name(), "cargo-geiger");
    }
}
