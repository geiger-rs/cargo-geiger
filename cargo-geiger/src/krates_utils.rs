use cargo::core::{Package, PackageId, PackageSet};
use cargo_metadata::Metadata;
use krates::Krates;
use std::path::PathBuf;

pub struct CargoMetadataParameters<'a> {
    pub krates: &'a Krates,
    pub metadata: &'a Metadata,
}

impl GetRoot for cargo_metadata::Package {
    fn get_root(&self) -> PathBuf {
        self.manifest_path.parent().unwrap().to_path_buf()
    }
}

impl ToCargoMetadataPackage for Package {
    fn to_cargo_metadata_package(
        &self,
        metadata: &Metadata,
    ) -> cargo_metadata::Package {
        metadata
            .packages
            .iter()
            .filter(|p| {
                p.name == self.name().to_string()
                    && p.version.major == self.version().major
                    && p.version.minor == self.version().minor
                    && p.version.patch == self.version().patch
            })
            .cloned()
            .collect::<Vec<cargo_metadata::Package>>()
            .pop()
            .unwrap()
    }
}

impl ToCargoMetadataPackageId for PackageId {
    fn to_cargo_metadata_package_id(
        &self,
        metadata: &Metadata,
    ) -> cargo_metadata::PackageId {
        metadata
            .packages
            .iter()
            .filter(|p| {
                p.name == self.name().to_string()
                    && p.version.major == self.version().major
                    && p.version.minor == self.version().minor
                    && p.version.patch == self.version().patch
            })
            .map(|p| p.id.clone())
            .collect::<Vec<cargo_metadata::PackageId>>()
            .pop()
            .unwrap()
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

pub trait GetRoot {
    fn get_root(&self) -> PathBuf;
}

pub trait ToCargoMetadataPackage {
    fn to_cargo_metadata_package(
        &self,
        metadata: &Metadata,
    ) -> cargo_metadata::Package;
}

pub trait ToCargoMetadataPackageId {
    fn to_cargo_metadata_package_id(
        &self,
        metadata: &Metadata,
    ) -> cargo_metadata::PackageId;
}

pub trait ToPackageId {
    fn to_package_id(
        &self,
        krates: &Krates,
        package_set: &PackageSet,
    ) -> PackageId;
}

#[cfg(test)]
mod krates_utils_tests {
    use super::*;

    use crate::cli::{get_registry, get_workspace, resolve};

    use cargo::Config;
    use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
    use krates::Builder;
    use rstest::*;
    use std::path::PathBuf;

    #[rstest]
    fn to_cargo_metadata_package_id_test() {
        let config = Config::default().unwrap();
        let manifest_path: Option<PathBuf> = None;
        let workspace = get_workspace(&config, manifest_path).unwrap();
        let package = workspace.current().unwrap();

        let metadata = construct_metadata();
        let cargo_metadata_package_id =
            package.package_id().to_cargo_metadata_package_id(&metadata);

        assert!(cargo_metadata_package_id.repr.contains("cargo-geiger"));
    }

    #[rstest]
    fn to_package_id_test() {
        let config = Config::default().unwrap();
        let manifest_path: Option<PathBuf> = None;
        let workspace = get_workspace(&config, manifest_path).unwrap();
        let package = workspace.current().unwrap();
        let mut registry = get_registry(&config, &package).unwrap();

        let features: Vec<String> = vec![];
        let all_features = false;
        let no_default_features = false;

        let (package_set, _) = resolve(
            package.package_id(),
            &mut registry,
            &workspace,
            &features,
            all_features,
            no_default_features,
        )
        .unwrap();

        let metadata = construct_metadata();
        let krates = Builder::new()
            .build_with_metadata(metadata.clone(), |_| ())
            .unwrap();

        let cargo_metadata_package = metadata.root_package().unwrap();
        let package_id = cargo_metadata_package
            .id
            .clone()
            .to_package_id(&krates, &package_set);

        assert_eq!(cargo_metadata_package.name, package_id.name().to_string());
    }

    fn construct_metadata() -> Metadata {
        MetadataCommand::new()
            .manifest_path("./Cargo.toml")
            .features(CargoOpt::AllFeatures)
            .exec()
            .unwrap()
    }
}
