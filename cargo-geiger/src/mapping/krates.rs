use crate::mapping::{
    GetLicenceFromCargoMetadataPackageId,
    GetPackageNameAndVersionFromCargoMetadataPackageId,
    GetRepositoryFromCargoMetadataPackageId, QueryResolve,
};

use krates::{Krates, PkgSpec};
use std::str::FromStr;

use cargo_metadata::PackageId as CargoMetadataPackageId;
use cargo_metadata::Version as CargoMetadataVersion;

impl GetLicenceFromCargoMetadataPackageId for Krates {
    fn get_licence_from_cargo_metadata_package_id(
        &self,
        package_id: &CargoMetadataPackageId,
    ) -> Option<String> {
        self.node_for_kid(package_id)
            .and_then(|package| package.krate.clone().license)
    }
}

impl GetPackageNameAndVersionFromCargoMetadataPackageId for Krates {
    fn get_package_name_and_version_from_cargo_metadata_package_id(
        &self,
        package_id: &CargoMetadataPackageId,
    ) -> Option<(String, CargoMetadataVersion)> {
        self.node_for_kid(package_id).map(|package| {
            (package.krate.clone().name, package.krate.clone().version)
        })
    }
}

impl GetRepositoryFromCargoMetadataPackageId for Krates {
    fn get_repository_from_cargo_metadata_package_id(
        &self,
        package_id: &CargoMetadataPackageId,
    ) -> Option<String> {
        self.node_for_kid(package_id)
            .and_then(|package| package.krate.clone().repository)
    }
}

impl QueryResolve for Krates {
    fn query_resolve(&self, query: &str) -> Option<CargoMetadataPackageId> {
        match PkgSpec::from_str(query) {
            Ok(package_spec) => self
                .krates_by_name(package_spec.name.as_str())
                .filter(|(_, node)| package_spec.matches(&node.krate))
                .map(|(_, node)| node.krate.clone().id)
                .collect::<Vec<CargoMetadataPackageId>>()
                .pop(),
            _ => {
                eprintln!("Failed to construct PkgSpec from string: {}", query);
                None
            }
        }
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
        let (package_name, _) = krates
            .get_package_name_and_version_from_cargo_metadata_package_id(
                &package.id,
            )
            .unwrap();
        assert_eq!(package_name, package.name);
    }

    #[rstest]
    fn get_package_version_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let (_, package_version) = krates
            .get_package_name_and_version_from_cargo_metadata_package_id(
                &package.id,
            )
            .unwrap();
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
            "cargo_metadata:0.12.3",
            "cargo_metadata",
            Version {
                major: 0,
                minor: 12,
                patch: 3,
                pre: vec![],
                build: vec![]
            }
        ),
        case(
            "cargo_metadata:0.12.3",
            "cargo_metadata",
            Version {
                major: 0,
                minor: 12,
                patch: 3,
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
        let package_id = krates.query_resolve(input_query_string).unwrap();

        let (package_name, package_version) = krates
            .get_package_name_and_version_from_cargo_metadata_package_id(
                &package_id,
            )
            .unwrap();

        assert_eq!(package_name, expected_package_name);
        assert_eq!(package_version, expected_package_version);
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
