pub mod geiger;
mod krates;
mod metadata;

use ::krates::Krates;
use cargo::core::dependency::DepKind;
use cargo_metadata::Metadata;
use std::collections::HashSet;
use std::path::PathBuf;

/// Holds a pointer to both a `Krates` graph, and the `Metadata` struct
/// which are often required together
pub struct CargoMetadataParameters<'a> {
    pub krates: &'a Krates,
    pub metadata: &'a Metadata,
}

pub trait DepsNotReplaced {
    fn deps_not_replaced(
        &self,
        package_id: cargo_metadata::PackageId,
    ) -> Option<
        Vec<(
            cargo_metadata::PackageId,
            HashSet<cargo_metadata::Dependency>,
        )>,
    >;
}

pub trait GetLicenceFromCargoMetadataPackageId {
    fn get_licence_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<String>;
}

pub trait GetPackageNameAndVersionFromCargoMetadataPackageId {
    fn get_package_name_and_version_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<(String, cargo_metadata::Version)>;
}

pub trait GetRepositoryFromCargoMetadataPackageId {
    fn get_repository_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<String>;
}

pub trait GetRoot {
    fn get_root(&self) -> Option<PathBuf>;
}

pub trait MatchesIgnoringSource {
    fn matches_ignoring_source(
        &self,
        krates: &Krates,
        package_id: cargo_metadata::PackageId,
    ) -> Option<bool>;
}

pub trait QueryResolve {
    fn query_resolve(&self, query: &str) -> Option<cargo_metadata::PackageId>;
}

pub trait ToCargoCoreDepKind {
    fn to_cargo_core_dep_kind(&self) -> DepKind;
}

pub trait ToCargoGeigerPackageId {
    fn to_cargo_geiger_package_id(
        &self,
        metadata: &Metadata,
    ) -> Option<cargo_geiger_serde::PackageId>;
}

pub trait ToCargoGeigerSource {
    fn to_cargo_geiger_source(
        &self,
        metadata: &Metadata,
    ) -> cargo_geiger_serde::Source;
}

pub trait ToCargoMetadataPackage {
    fn to_cargo_metadata_package(
        &self,
        metadata: &Metadata,
    ) -> Option<cargo_metadata::Package>;
}

pub trait ToCargoMetadataPackageId {
    fn to_cargo_metadata_package_id(
        &self,
        metadata: &Metadata,
    ) -> Option<cargo_metadata::PackageId>;
}
