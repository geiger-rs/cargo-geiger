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
use cargo::util::{self, important_paths, CargoResult};
use cargo::Config;
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
use cargo_platform::Cfg;
use krates::Builder as KratesBuilder;
use krates::Krates;
use std::path::PathBuf;
use std::str::{self, FromStr};

pub fn get_cargo_metadata(
    args: &Args,
    config: &Config,
) -> CargoResult<Metadata> {
    let root_manifest_path = match args.manifest_path.clone() {
        Some(path) => path,
        None => important_paths::find_root_manifest_for_wd(config.cwd())?,
    };

    let mut metadata_command = MetadataCommand::new();
    metadata_command.manifest_path(root_manifest_path);

    if args.features_args.all_features {
        metadata_command.features(CargoOpt::AllFeatures);
    }

    if args.features_args.no_default_features {
        metadata_command.features(CargoOpt::NoDefaultFeatures);
    }

    if !args.features_args.features.is_empty() {
        metadata_command.features(CargoOpt::SomeFeatures(
            args.features_args.features.clone(),
        ));
    }

    Ok(metadata_command.exec()?)
}

/// TODO: Write proper documentation for this.
/// This function seems to be looking up the active flags for conditional
/// compilation (`cargo_platform::Cfg` instances).
pub fn get_cfgs(
    config: &Config,
    target: &Option<String>,
    workspace: &Workspace,
) -> CargoResult<Option<Vec<Cfg>>> {
    let mut process =
        util::process(&config.load_global_rustc(Some(workspace))?.path);
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

pub fn get_krates(cargo_metadata: &Metadata) -> CargoResult<Krates> {
    Ok(KratesBuilder::new()
        .build_with_metadata(cargo_metadata.clone(), |_| ())?)
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

// TODO: Make a wrapper type for canonical paths and hide all mutable access.

#[cfg(test)]
mod cli_tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn get_cargo_metadata_test() {
        let args = Args::default();
        let config = Config::default().unwrap();

        let cargo_metadata_result = get_cargo_metadata(&args, &config);

        assert!(cargo_metadata_result.is_ok());
    }

    #[rstest]
    fn get_cfgs_test() {
        let config = Config::default().unwrap();
        let target: Option<String> = None;
        let root =
            important_paths::find_root_manifest_for_wd(config.cwd()).unwrap();
        let workspace = Workspace::new(&root, &config).unwrap();

        let cfgs = get_cfgs(&config, &target, &workspace);

        assert!(cfgs.is_ok());
        let cfg_vec_option = cfgs.unwrap();
        assert!(cfg_vec_option.is_some());
        let cfg_vec = cfg_vec_option.unwrap();

        let mut names =
            cfg_vec.iter().filter(|cfg| matches!(cfg, Cfg::Name(_)));

        let mut key_pairs = cfg_vec
            .iter()
            .filter(|cfg| matches!(cfg, Cfg::KeyPair(_, _)));

        assert!(!names.next().is_none());
        assert!(!key_pairs.next().is_none());
    }

    #[rstest]
    fn get_krates_test() {
        let args = Args::default();
        let config = Config::default().unwrap();
        let cargo_metadata = get_cargo_metadata(&args, &config).unwrap();

        let krates_result = get_krates(&cargo_metadata);
        assert!(krates_result.is_ok());
    }

    #[rstest]
    fn get_workspace_test() {
        let config = Config::default().unwrap();
        let manifest_path: Option<PathBuf> = None;

        let workspace_cargo_result = get_workspace(&config, manifest_path);
        assert!(workspace_cargo_result.is_ok());
        let workspace = workspace_cargo_result.unwrap();

        let package_result = workspace.current();
        assert!(package_result.is_ok());
        let package = package_result.unwrap();

        assert_eq!(package.package_id().name(), "cargo-geiger");
    }
}
