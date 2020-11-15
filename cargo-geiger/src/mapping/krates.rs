use crate::mapping::{
    GetManifestMetadataFromCargoMetadataPackageId,
    GetPackageNameFromCargoMetadataPackageId,
    GetPackageVersionFromCargoMetadataPackageId, ToPackage,
};

use cargo::core::manifest::ManifestMetadata;
use cargo::core::PackageSet;
use krates::Krates;

impl GetManifestMetadataFromCargoMetadataPackageId for Krates {
    fn get_manifest_metadata_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
        package_set: &PackageSet,
    ) -> ManifestMetadata {
        let package = package_id.to_package(self, package_set);
        package.manifest().metadata().clone()
    }
}

impl GetPackageNameFromCargoMetadataPackageId for Krates {
    fn get_package_name_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> String {
        let package = self.node_for_kid(package_id);
        package.unwrap().krate.clone().name
    }
}

impl GetPackageVersionFromCargoMetadataPackageId for Krates {
    fn get_package_version_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> cargo_metadata::Version {
        let package = self.node_for_kid(package_id);
        package.unwrap().krate.clone().version
    }
}

#[cfg(test)]
mod krates_tests {
    use super::*;

    use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
    use krates::Builder as KratesBuilder;
    use rstest::*;

    #[rstest]
    fn get_package_name_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let package_name =
            krates.get_package_name_from_cargo_metadata_package_id(&package.id);
        assert_eq!(package_name, package.name);
    }

    #[rstest]
    fn get_package_version_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let package_version = krates
            .get_package_version_from_cargo_metadata_package_id(&package.id);
        assert_eq!(package_version, package.version);
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
