use super::{
    CargoMetadataParameters, DepsNotReplaced,
    GetPackageNameFromCargoMetadataPackageId,
    GetPackageVersionFromCargoMetadataPackageId, GetRoot,
    MatchesIgnoringSource, Replacement, ToCargoCoreDepKind,
    ToCargoMetadataPackageId, ToPackageId,
};

use crate::utils::ToPackage;
use cargo::core::dependency::DepKind;
use cargo::core::{Package, PackageId, PackageSet, Resolve};
use cargo_metadata::DependencyKind;
use krates::Krates;
use std::collections::HashSet;
use std::path::PathBuf;

impl DepsNotReplaced for cargo_metadata::Metadata {
    fn deps_not_replaced(
        &self,
        krates: &Krates,
        package_id: cargo_metadata::PackageId,
        package_set: &PackageSet,
        resolve: &Resolve,
    ) -> Vec<(
        cargo_metadata::PackageId,
        HashSet<cargo_metadata::Dependency>,
    )> {
        let cargo_core_package_id =
            package_id.to_package_id(krates, package_set);
        let deps_not_replaced =
            resolve.deps_not_replaced(cargo_core_package_id);

        let mut cargo_metadata_deps_not_replaced = vec![];

        for (dep_package_id, _) in deps_not_replaced {
            cargo_metadata_deps_not_replaced.push((
                dep_package_id.to_cargo_metadata_package_id(self),
                HashSet::<cargo_metadata::Dependency>::new(),
            ))
        }

        cargo_metadata_deps_not_replaced
    }
}

impl GetRoot for cargo_metadata::Package {
    fn get_root(&self) -> PathBuf {
        self.manifest_path.parent().unwrap().to_path_buf()
    }
}

impl MatchesIgnoringSource for cargo_metadata::Dependency {
    fn matches_ignoring_source(
        &self,
        krates: &Krates,
        package_id: cargo_metadata::PackageId,
    ) -> bool {
        self.name
            == krates
                .get_package_name_from_cargo_metadata_package_id(&package_id)
            && self.req.matches(
                &krates.get_package_version_from_cargo_metadata_package_id(
                    &package_id,
                ),
            )
    }
}

impl Replacement for cargo_metadata::PackageId {
    fn replace(
        &self,
        cargo_metadata_parameters: &CargoMetadataParameters,
        package_set: &PackageSet,
        resolve: &Resolve,
    ) -> cargo_metadata::PackageId {
        let package_id =
            self.to_package_id(cargo_metadata_parameters.krates, package_set);
        match resolve.replacement(package_id) {
            Some(id) => id.to_cargo_metadata_package_id(
                cargo_metadata_parameters.metadata,
            ),
            None => self.clone(),
        }
    }
}

impl ToCargoCoreDepKind for DependencyKind {
    fn to_cargo_core_dep_kind(&self) -> DepKind {
        match self {
            DependencyKind::Build => DepKind::Build,
            DependencyKind::Development => DepKind::Development,
            DependencyKind::Normal => DepKind::Normal,
            _ => panic!("Unknown dependency kind"),
        }
    }
}

impl ToPackage for cargo_metadata::PackageId {
    fn to_package(&self, krates: &Krates, package_set: &PackageSet) -> Package {
        let package_id = self.to_package_id(krates, package_set);
        package_set
            .get_one(package_id)
            .unwrap_or_else(|_| {
                // TODO: Avoid panic, return Result.
                panic!("Expected to find package by id: {}", package_id);
            })
            .clone()
    }
}

impl ToPackageId for cargo_metadata::PackageId {
    fn to_package_id(
        &self,
        krates: &Krates,
        package_set: &PackageSet,
    ) -> PackageId {
        let node = krates.node_for_kid(&self).unwrap();
        package_set
            .package_ids()
            .filter(|p| {
                p.name().to_string() == node.krate.name
                    && p.version().major == node.krate.version.major
                    && p.version().minor == node.krate.version.minor
                    && p.version().patch == node.krate.version.patch
            })
            .collect::<Vec<PackageId>>()
            .pop()
            .unwrap()
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    use super::super::GetPackageNameFromCargoMetadataPackageId;

    use crate::args::FeaturesArgs;
    use crate::cli::{get_registry, get_workspace, resolve};

    use cargo::Config;
    use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
    use krates::Builder as KratesBuilder;
    use rstest::*;
    use std::path::PathBuf;

    #[rstest]
    fn deps_not_replaced_test() {
        let args = FeaturesArgs::default();
        let config = Config::default().unwrap();
        let manifest_path: Option<PathBuf> = None;
        let workspace = get_workspace(&config, manifest_path).unwrap();
        let package = workspace.current().unwrap();
        let mut registry = get_registry(&config, &package).unwrap();

        let (package_set, resolve) =
            resolve(&args, package.package_id(), &mut registry, &workspace)
                .unwrap();

        let (krates, metadata) = construct_krates_and_metadata();
        let cargo_metadata_package_id =
            package.package_id().to_cargo_metadata_package_id(&metadata);

        let deps_not_replaced = resolve.deps_not_replaced(package.package_id());
        let cargo_metadata_deps_not_replaced = metadata.deps_not_replaced(
            &krates,
            cargo_metadata_package_id,
            &package_set,
            &resolve,
        );

        let mut cargo_core_package_names = deps_not_replaced
            .map(|(p, _)| p.name().to_string())
            .collect::<Vec<String>>();

        let mut cargo_metadata_package_names = cargo_metadata_deps_not_replaced
            .iter()
            .map(|(p, _)| {
                krates.get_package_name_from_cargo_metadata_package_id(p)
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
        let package_root = package.get_root();
        assert_eq!(
            package_root,
            package.manifest_path.parent().unwrap().to_path_buf()
        );
    }

    #[rstest]
    fn matches_ignoring_source_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();

        let dependency = package.dependencies.clone().pop().unwrap();

        assert_eq!(
            dependency.matches_ignoring_source(&krates, package.clone().id),
            false
        );

        let dependency_package_id = krates
            .krates()
            .filter(|k| {
                k.krate.name == dependency.name
                    && dependency.req.matches(&k.krate.version)
            })
            .map(|k| k.id.clone())
            .collect::<Vec<cargo_metadata::PackageId>>()
            .pop()
            .unwrap();

        assert!(
            dependency.matches_ignoring_source(&krates, dependency_package_id),
            true
        );
    }

    #[rstest]
    fn replace_test() {
        let args = FeaturesArgs::default();
        let config = Config::default().unwrap();
        let manifest_path: Option<PathBuf> = None;
        let workspace = get_workspace(&config, manifest_path).unwrap();
        let package = workspace.current().unwrap();
        let mut registry = get_registry(&config, &package).unwrap();

        let (package_set, resolve) =
            resolve(&args, package.package_id(), &mut registry, &workspace)
                .unwrap();

        let (krates, metadata) = construct_krates_and_metadata();
        let cargo_metadata_package_id =
            package.package_id().to_cargo_metadata_package_id(&metadata);
        let cargo_metadata_parameters = CargoMetadataParameters {
            krates: &krates,
            metadata: &metadata,
        };

        assert_eq!(
            cargo_metadata_package_id,
            cargo_metadata_package_id.replace(
                &cargo_metadata_parameters,
                &package_set,
                &resolve
            )
        )
    }

    #[rstest(
        input_dependency_kind,
        expected_dep_kind,
        case(DependencyKind::Build, DepKind::Build),
        case(DependencyKind::Development, DepKind::Development),
        case(DependencyKind::Normal, DepKind::Normal)
    )]
    fn to_cargo_core_dep_kind(
        input_dependency_kind: DependencyKind,
        expected_dep_kind: DepKind,
    ) {
        assert_eq!(
            input_dependency_kind.to_cargo_core_dep_kind(),
            expected_dep_kind
        )
    }

    #[rstest]
    fn to_package_id_test() {
        let args = FeaturesArgs::default();
        let config = Config::default().unwrap();
        let manifest_path: Option<PathBuf> = None;
        let workspace = get_workspace(&config, manifest_path).unwrap();
        let package = workspace.current().unwrap();
        let mut registry = get_registry(&config, &package).unwrap();

        let (package_set, _) =
            resolve(&args, package.package_id(), &mut registry, &workspace)
                .unwrap();

        let (krates, metadata) = construct_krates_and_metadata();

        let cargo_metadata_package = metadata.root_package().unwrap();
        let package_id = cargo_metadata_package
            .id
            .clone()
            .to_package_id(&krates, &package_set);

        assert_eq!(cargo_metadata_package.name, package_id.name().to_string());
    }

    fn construct_krates_and_metadata() -> (Krates, Metadata) {
        let metadata = MetadataCommand::new()
            .manifest_path("./Cargo.toml")
            .features(CargoOpt::AllFeatures)
            .exec()
            .unwrap();

        let krates = KratesBuilder::new()
            .build_with_metadata(metadata.clone(), |_| ())
            .unwrap();

        (krates, metadata)
    }
}
