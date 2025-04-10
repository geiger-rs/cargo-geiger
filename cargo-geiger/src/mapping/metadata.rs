pub mod dependency;
pub mod package;
pub mod package_id;

use super::{
    DepsNotReplaced, GetPackageIdInformation, MatchesIgnoringSource,
    ToCargoGeigerPackageId, ToCargoMetadataPackageId,
};
use package_id::ToCargoMetadataPackage;

use crate::mapping::krates_mapping::GetPackage;
use crate::mapping::{ToCargoGeigerDependencyKind, ToCargoGeigerSource};

use krates::cm::Metadata;
use std::collections::HashSet;
use std::fmt::Display;
use std::slice::Iter;

use krates::cm::Dependency as CargoMetadataDependency;
use krates::cm::DependencyKind as CargoMetadataDependencyKind;
use krates::cm::Package as CargoMetadataPackage;
use krates::cm::PackageId as CargoMetadataPackageId;

use cargo_geiger_serde::DependencyKind as CargoGeigerSerdeDependencyKind;

impl DepsNotReplaced for Metadata {
    fn deps_not_replaced<T: ToCargoMetadataPackage + Display>(
        &self,
        package_id: &T,
        is_root_package: bool,
    ) -> Option<Vec<(CargoMetadataPackageId, HashSet<CargoMetadataDependency>)>>
    {
        let mut cargo_metadata_deps_not_replaced = vec![];
        let mut package_id_hashset = HashSet::<CargoMetadataPackageId>::new();

        match package_id.to_cargo_metadata_package(self) {
            Some(package) => {
                for dependency in package.dependencies {
                    if let Some(package_id) =
                        dependency.to_cargo_metadata_package_id(self)
                    {
                        if dependency.kind
                            == krates::cm::DependencyKind::Development
                            && !is_root_package
                        {
                            continue;
                        }
                        if !package_id_hashset.contains(&package_id) {
                            cargo_metadata_deps_not_replaced.push((
                                package_id.clone(),
                                HashSet::<CargoMetadataDependency>::new(),
                            ));
                            package_id_hashset.insert(package_id);
                        }
                    }
                }
                Some(cargo_metadata_deps_not_replaced)
            }
            None => {
                eprintln!("Failed to convert Package Id: {} to Cargo Metadata Package", package_id);
                None
            }
        }
    }
}

impl MatchesIgnoringSource for CargoMetadataDependency {
    fn matches_ignoring_source<
        T: GetPackage,
        U: GetPackageIdInformation + Display,
    >(
        &self,
        krates: &T,
        package_id: &U,
    ) -> Option<bool> {
        match package_id.get_package_id_name_and_version(krates) {
            Some((name, version)) => {
                Some(name == self.name && self.req.matches(&version))
            }
            _ => {
                eprintln!(
                    "Failed to match (ignoring source) package: {} ",
                    package_id
                );
                None
            }
        }
    }
}

impl ToCargoGeigerDependencyKind for CargoMetadataDependencyKind {
    fn to_cargo_geiger_dependency_kind(
        &self,
    ) -> Option<CargoGeigerSerdeDependencyKind> {
        match self {
            CargoMetadataDependencyKind::Build => {
                Some(CargoGeigerSerdeDependencyKind::Build)
            }
            CargoMetadataDependencyKind::Development => {
                Some(CargoGeigerSerdeDependencyKind::Development)
            }
            CargoMetadataDependencyKind::Normal => {
                Some(CargoGeigerSerdeDependencyKind::Normal)
            }
            _ => {
                eprintln!("Unrecognised Dependency Kind");
                None
            }
        }
    }
}

impl ToCargoGeigerPackageId for CargoMetadataPackageId {
    fn to_cargo_geiger_package_id(
        &self,
        metadata: &Metadata,
    ) -> Option<cargo_geiger_serde::PackageId> {
        match self.to_cargo_metadata_package(metadata) {
            Some(package) => {
                let metadata_source = self.to_cargo_geiger_source(metadata);
                Some(cargo_geiger_serde::PackageId {
                    name: package.name,
                    version: package.version,
                    source: metadata_source,
                })
            }
            None => {
                eprintln!("Failed to convert PackageId: {} to Package", self);
                None
            }
        }
    }
}

impl ToCargoMetadataPackageId for CargoMetadataDependency {}

pub trait GetMetadataPackages {
    fn get_metadata_packages(&self) -> Iter<CargoMetadataPackage>;
}

impl GetMetadataPackages for Metadata {
    fn get_metadata_packages(&self) -> Iter<CargoMetadataPackage> {
        self.packages.iter()
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    use super::super::{GetPackageIdInformation, ToCargoCoreDepKind};

    use crate::args::FeaturesArgs;
    use crate::cli::get_workspace;
    use crate::lib_tests::construct_krates_and_metadata;

    use crate::mapping::metadata::dependency::GetDependencyInformation;
    use crate::mapping::GetPackageRoot;

    use cargo::core::dependency::DepKind;
    use cargo::core::registry::PackageRegistry;
    use cargo::core::resolver::features::CliFeatures;
    use cargo::core::resolver::features::HasDevUnits;
    use cargo::core::{
        Package, PackageId, PackageIdSpec, PackageSet, Resolve, Workspace,
    };
    use cargo::sources::SourceConfigMap;
    use cargo::{ops, CargoResult, GlobalContext};
    use krates::semver::VersionReq;
    use rstest::*;
    use std::path::PathBuf;

    #[rstest]
    fn deps_not_replaced_test() {
        let args = FeaturesArgs::default();
        let gctx = GlobalContext::default().unwrap();
        let (package, mut registry, workspace) =
            construct_package_registry_workspace_tuple(&gctx);

        let (_, resolve) =
            resolve(&args, package.package_id(), &mut registry, &workspace)
                .unwrap();

        let (krates, metadata) = construct_krates_and_metadata();
        let cargo_metadata_package_id = package
            .package_id()
            .to_cargo_metadata_package_id(&metadata)
            .unwrap();

        let deps_not_replaced = resolve.deps_not_replaced(package.package_id());
        let cargo_metadata_deps_not_replaced = metadata
            .deps_not_replaced(&cargo_metadata_package_id, true)
            .unwrap();

        let mut cargo_core_package_names = deps_not_replaced
            .map(|(p, _)| p.name().to_string())
            .collect::<Vec<String>>();

        let mut cargo_metadata_package_names = cargo_metadata_deps_not_replaced
            .iter()
            .map(|(p, _)| {
                let (name, _) =
                    p.get_package_id_name_and_version(&krates).unwrap();
                name
            })
            .collect::<Vec<String>>();

        cargo_core_package_names.sort();
        cargo_metadata_package_names.sort();

        assert_eq!(cargo_core_package_names, cargo_metadata_package_names);
    }

    #[rstest]
    fn get_root_test() {
        let (_, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let package_root = package.get_root().unwrap();
        assert_eq!(
            package_root,
            package.manifest_path.parent().unwrap().to_path_buf()
        );
    }

    #[rstest]
    fn matches_ignoring_source() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();

        let dependency = package.dependencies.clone().pop().unwrap();

        assert!(!dependency
            .matches_ignoring_source(&krates, &package.clone().id)
            .unwrap());

        let dependency_package_id = krates
            .krates()
            .filter(|k| {
                k.name == dependency.name && dependency.req.matches(&k.version)
            })
            .map(|k| k.id.clone())
            .collect::<Vec<krates::cm::PackageId>>()
            .pop()
            .unwrap();

        assert!(dependency
            .matches_ignoring_source(&krates, &dependency_package_id)
            .unwrap());
    }

    #[rstest(
        input_dependency_kind,
        expected_dep_kind,
        case(CargoMetadataDependencyKind::Build, DepKind::Build),
        case(CargoMetadataDependencyKind::Development, DepKind::Development),
        case(CargoMetadataDependencyKind::Normal, DepKind::Normal)
    )]
    fn to_cargo_core_dep_kind(
        input_dependency_kind: CargoMetadataDependencyKind,
        expected_dep_kind: DepKind,
    ) {
        assert_eq!(
            input_dependency_kind.to_cargo_core_dep_kind(),
            expected_dep_kind
        )
    }

    #[rstest]
    fn to_cargo_geiger_package_id_test() {
        let (_, metadata) = construct_krates_and_metadata();

        let root_package = metadata.root_package().unwrap();

        let cargo_geiger_package_id = root_package
            .id
            .to_cargo_geiger_package_id(&metadata)
            .unwrap();

        assert_eq!(cargo_geiger_package_id.name, root_package.name);

        assert_eq!(
            cargo_geiger_package_id.version.major,
            root_package.version.major
        );
        assert_eq!(
            cargo_geiger_package_id.version.minor,
            root_package.version.minor
        );
        assert_eq!(
            cargo_geiger_package_id.version.patch,
            root_package.version.patch
        );
    }

    fn construct_package_registry_workspace_tuple(
        gctx: &GlobalContext,
    ) -> (Package, PackageRegistry, Workspace) {
        let manifest_path: Option<PathBuf> = None;
        let workspace = get_workspace(gctx, manifest_path).unwrap();
        let package = workspace.current().unwrap().clone();
        let registry = get_registry(gctx, &package).unwrap();

        (package, registry, workspace)
    }

    fn get_registry<'a>(
        gctx: &'a GlobalContext,
        package: &Package,
    ) -> CargoResult<PackageRegistry<'a>> {
        let mut registry = PackageRegistry::new_with_source_config(
            gctx,
            SourceConfigMap::new(gctx).unwrap(),
        )?;
        registry.add_sources(Some(package.package_id().source_id()))?;
        Ok(registry)
    }

    fn resolve<'a, 'cfg>(
        args: &FeaturesArgs,
        package_id: PackageId,
        registry: &mut PackageRegistry<'cfg>,
        workspace: &'a Workspace<'cfg>,
    ) -> CargoResult<(PackageSet<'a>, Resolve)> {
        let uses_default_features = !args.no_default_features;

        let cli_features = CliFeatures::from_command_line(
            &args.features,
            args.all_features,
            uses_default_features,
        )
        .unwrap();

        let prev = ops::load_pkg_lockfile(workspace)?;
        let package_id_spec = PackageIdSpec::new(package_id.name().to_string())
            .with_version(package_id.version().clone().into())
            .with_url(package_id.source_id().url().clone());
        let resolve = ops::resolve_with_previous(
            registry,
            workspace,
            &cli_features,
            HasDevUnits::Yes,
            prev.as_ref(),
            None,
            &[package_id_spec],
            true,
        )?;
        let packages = ops::get_resolved_packages(
            &resolve,
            PackageRegistry::new_with_source_config(
                workspace.gctx(),
                SourceConfigMap::new(workspace.gctx()).unwrap(),
            )?,
        )?;
        Ok((packages, resolve))
    }

    impl ToCargoCoreDepKind for CargoMetadataDependencyKind {
        fn to_cargo_core_dep_kind(&self) -> DepKind {
            match self {
                CargoMetadataDependencyKind::Build => DepKind::Build,
                CargoMetadataDependencyKind::Development => {
                    DepKind::Development
                }
                CargoMetadataDependencyKind::Normal => DepKind::Normal,
                _ => panic!("Unknown dependency kind"),
            }
        }
    }

    impl ToCargoMetadataPackageId for PackageId {}

    impl GetDependencyInformation for PackageId {
        fn get_dependency_name(&self) -> String {
            self.name().clone().to_string()
        }
        fn get_dependency_version_req(&self) -> VersionReq {
            VersionReq::parse(&self.version().clone().to_string()).unwrap()
        }
    }
}
