use crate::mapping::QueryResolve;

use krates::{Kid, Krates, Node, PkgSpec};
use std::str::FromStr;

use krates::cm::{Package, PackageId as CargoMetadataPackageId};

pub trait GetPackage {
    fn get_package(
        &self,
        package_id: &CargoMetadataPackageId,
    ) -> Option<&Package>;
}

impl GetPackage for Krates {
    fn get_package(
        &self,
        package_id: &CargoMetadataPackageId,
    ) -> Option<&Package> {
        let kid: Kid = package_id.clone().into();
        let node = self.node_for_kid(&kid)?;
        match node {
            Node::Krate { krate, .. } => Some(krate),
            Node::Feature { .. } => None,
        }
    }
}

impl QueryResolve for Krates {
    fn query_resolve(&self, query: &str) -> Option<CargoMetadataPackageId> {
        match PkgSpec::from_str(query) {
            Ok(package_spec) => self
                .krates_by_name(package_spec.name.as_str())
                .filter(|m| package_spec.matches(&m.krate))
                .map(|m| m.krate.clone().id)
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

    use crate::lib_tests::construct_krates_and_metadata;
    use crate::mapping::GetPackageIdInformation;

    use krates::semver::Version;
    use rstest::*;
    use semver::{BuildMetadata, Prerelease};

    #[rstest]
    fn get_licence_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let licence_option = &package.id.get_package_id_licence(&krates);
        assert!(licence_option.as_ref().is_some());
        let licence = licence_option.as_ref().unwrap();
        assert_eq!(licence, &String::from("Apache-2.0 OR MIT"))
    }

    #[rstest]
    fn get_package_name_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let (package_name, _) =
            package.id.get_package_id_name_and_version(&krates).unwrap();
        assert_eq!(package_name, package.name);
    }

    #[rstest]
    fn get_package_version_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let (_, package_version) =
            package.id.get_package_id_name_and_version(&krates).unwrap();
        assert_eq!(package_version, package.version);
    }

    #[rstest]
    fn get_repository_from_cargo_metadata_package_id_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let repository_option = &package.id.get_package_id_repository(&krates);
        assert!(repository_option.is_some());
        let repository = repository_option.as_ref().unwrap();
        assert_eq!(
            repository,
            &String::from("https://github.com/rust-secure-code/cargo-geiger")
        );
    }

    #[rstest(
        input_query_string,
        expected_package_name,
        expected_package_version,
        case(
            "krates:0.18.1",
            "krates",
            Version {
                major: 0,
                minor: 18,
                patch: 1,
                pre: Prerelease::EMPTY,
                build: BuildMetadata::EMPTY
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

        let (package_name, package_version) =
            package_id.get_package_id_name_and_version(&krates).unwrap();

        assert_eq!(package_name, expected_package_name);
        assert_eq!(package_version, expected_package_version);
    }
}
