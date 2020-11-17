use crate::mapping::{
    GetLicenceFromCargoMetadataPackageId,
    GetPackageNameFromCargoMetadataPackageId,
    GetPackageVersionFromCargoMetadataPackageId,
    GetRepositoryFromCargoMetadataPackageId, QueryResolve,
};

use krates::{Krates, PkgSpec};
use std::str::FromStr;

impl GetLicenceFromCargoMetadataPackageId for Krates {
    fn get_licence_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<String> {
        let package = self.node_for_kid(package_id);
        package.unwrap().krate.clone().license
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

impl GetRepositoryFromCargoMetadataPackageId for Krates {
    fn get_repository_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<String> {
        let package = self.node_for_kid(package_id);
        package.unwrap().krate.clone().repository
    }
}

impl QueryResolve for Krates {
    fn query_resolve(&self, query: &str) -> cargo_metadata::PackageId {
        let package_spec = PkgSpec::from_str(query).unwrap();
        self.krates_by_name(package_spec.name.as_str())
            .filter(|(_, node)| package_spec.matches(&node.krate))
            .map(|(_, node)| node.krate.clone())
            .collect::<Vec<cargo_metadata::Package>>()
            .pop()
            .unwrap()
            .id
    }
}

#[cfg(test)]
mod krates_tests {
    use super::*;

    use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Version};
    use krates::Builder as KratesBuilder;
    use rstest::*;

    #[rstest]
    fn get_licence_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let licence_option =
            krates.get_licence_from_cargo_metadata_package_id(&package.id);
        assert!(licence_option.is_some());
        let licence = licence_option.unwrap();
        assert_eq!(licence, String::from("Apache-2.0/MIT"))
    }

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

    #[rstest]
    fn get_repository_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let repository_option =
            krates.get_repository_from_cargo_metadata_package_id(&package.id);
        assert!(repository_option.is_some());
        let repository = repository_option.unwrap();
        assert_eq!(
            repository,
            String::from("https://github.com/rust-secure-code/cargo-geiger")
        );
    }

    #[rstest(
        input_query_string,
        expected_package_name,
        expected_package_version,
        case(
            "cargo_metadata:0.12.0",
            "cargo_metadata",
            Version {
                major: 0,
                minor: 12,
                patch: 0,
                pre: vec![],
                build: vec![]
            }
        ),
        case(
            "cargo_metadata:0.12.0",
            "cargo_metadata",
            Version {
                major: 0,
                minor: 12,
                patch: 0,
                pre: vec![],
                build: vec![]
            }
        )
    )]
    fn query_resolve_test(
        input_query_string: &str,
        expected_package_name: &str,
        expected_package_version: Version,
    ) {
        let (krates, _) = construct_krates_and_metadata();
        let package_id = krates.query_resolve(input_query_string);

        assert_eq!(
            krates.get_package_name_from_cargo_metadata_package_id(&package_id),
            expected_package_name
        );

        assert_eq!(
            krates.get_package_version_from_cargo_metadata_package_id(
                &package_id
            ),
            expected_package_version
        );
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
