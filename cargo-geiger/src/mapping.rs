mod geiger;
mod krates;
mod metadata;

use metadata::package_id::ToCargoMetadataPackage;
use metadata::package::{GetPackageName, GetPackageParent, GetPackageVersion};

use ::krates::Krates;
use cargo::core::dependency::DepKind;
use cargo_metadata::Metadata;
use std::collections::HashSet;
use std::fmt::Display;
use std::path::PathBuf;

use cargo_metadata::Dependency as CargoMetadataDependency;
use cargo_metadata::PackageId as CargoMetadataPackageId;
use cargo_metadata::Version as CargoMetadataVersion;

use cargo_geiger_serde::DependencyKind as CargoGeigerSerdeDependencyKind;
use cargo_geiger_serde::PackageId as CargoGeigerSerdePackageId;
use cargo_geiger_serde::Source as CargoGeigerSerdeSource;
use crate::mapping::metadata::GetMetadataPackages;
use crate::mapping::metadata::dependency::{GetDependencyName, GetDependencyRequirement};

/// Holds a pointer to both a `Krates` graph, and the `Metadata` struct
/// which are often required together
pub struct CargoMetadataParameters<'a> {
    pub krates: &'a Krates,
    pub metadata: &'a Metadata,
}

pub trait DepsNotReplaced {
    fn deps_not_replaced<T: ToCargoMetadataPackage + Display>(
        &self,
        package_id: &T,
    ) -> Option<
        Vec<(
            CargoMetadataPackageId,
            HashSet<CargoMetadataDependency>,
        )>,
    >;
}

pub trait GetLicenceFromCargoMetadataPackageId {
    fn get_licence_from_cargo_metadata_package_id(
        &self,
        package_id: &CargoMetadataPackageId,
    ) -> Option<String>;
}


pub trait GetPackageRoot : GetPackageName + GetPackageParent + GetPackageVersion {
    fn get_root(&self) -> Option<PathBuf> {
        match self.get_package_parent() {
            Some(path) => Some(path.to_path_buf()),
            None => {
                eprintln!(
                    "Failed to get root for: {} {:?}",
                    self.get_package_name(), self.get_package_version()
                );
                None
            }
        }
    }
}

pub trait GetPackageNameAndVersionFromCargoMetadataPackageId {
    fn get_package_name_and_version_from_cargo_metadata_package_id(
        &self,
        package_id: &CargoMetadataPackageId,
    ) -> Option<(String, CargoMetadataVersion)>;
}

pub trait GetRepositoryFromCargoMetadataPackageId {
    fn get_repository_from_cargo_metadata_package_id(
        &self,
        package_id: &CargoMetadataPackageId,
    ) -> Option<String>;
}

pub trait MatchesIgnoringSource {
    fn matches_ignoring_source(
        &self,
        krates: &Krates,
        package_id: CargoMetadataPackageId,
    ) -> Option<bool>;
}

pub trait QueryResolve {
    fn query_resolve(&self, query: &str) -> Option<CargoMetadataPackageId>;
}

pub trait ToCargoCoreDepKind {
    fn to_cargo_core_dep_kind(&self) -> DepKind;
}

pub trait ToCargoGeigerDependencyKind {
    fn to_cargo_geiger_dependency_kind(
        &self,
    ) -> Option<CargoGeigerSerdeDependencyKind>;
}

pub trait ToCargoGeigerPackageId {
    fn to_cargo_geiger_package_id(
        &self,
        metadata: &Metadata,
    ) -> Option<CargoGeigerSerdePackageId>;
}

pub trait ToCargoGeigerSource {
    fn to_cargo_geiger_source(
        &self,
        metadata: &Metadata,
    ) -> CargoGeigerSerdeSource;
}


pub trait ToCargoMetadataPackageId: GetDependencyName + GetDependencyRequirement {
    fn to_cargo_metadata_package_id<T: GetMetadataPackages>(
        &self,
        metadata: &T,
    ) -> Option<CargoMetadataPackageId> {
        metadata.get_metadata_packages()
            .filter(|p| p.name == self.get_dependency_name() && self.get_dependency_requirement().matches(&p.version))
            .map(|p| p.id.clone())
            .collect::<Vec<CargoMetadataPackageId>>()
            .pop()
    }
}

#[cfg(test)]
mod mapping_tests {
    use super::*;

    use std::path::Path;
    use rstest::*;

    struct MockPackage<'a> {
        mock_package_name: String,
        mock_package_parent: Option<&'a Path>,
        mock_package_version: CargoMetadataVersion,
    }

    impl GetPackageName for MockPackage<'_> {
        fn get_package_name(&self) -> String {
            self.mock_package_name.clone()
        }
    }

    impl GetPackageParent for MockPackage<'_> {
        fn get_package_parent(&self) -> Option<&Path> {
            self.mock_package_parent
        }
    }

    impl GetPackageVersion for MockPackage<'_> {
        fn get_package_version(&self) -> CargoMetadataVersion {
            self.mock_package_version.clone()
        }
    }

    impl GetPackageRoot for MockPackage<'_> {}

    #[rstest(
        input_package_path_option,
        expected_package_path_buf_option,
        case(
            Some(Path::new("/path/to/file")),
            Some(PathBuf::from("/path/to/file"))
        ),
        case(
            None,
            None
        )
    )]
    fn get_package_root_test(
        input_package_path_option: Option<&Path>,
        expected_package_path_buf_option: Option<PathBuf>
    ) {
        let _mock_package_parent = match input_package_path_option {
            Some(path) => Some(path),
            None => None
        };

        let mock_package = MockPackage {
            mock_package_name: String::from("package_name"),
            mock_package_parent: input_package_path_option,
            mock_package_version: CargoMetadataVersion::new(1,1,1)
        };

        assert_eq!(
            mock_package.get_root(),
            expected_package_path_buf_option
        )
    }
}
