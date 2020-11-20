mod core;
mod krates;
mod metadata;

use ::krates::Krates;
use cargo::core::dependency::DepKind;
use cargo::core::{Package, PackageId, PackageSet, Resolve};
use cargo_metadata::{DependencyKind, Metadata};
use std::collections::HashSet;
use std::path::PathBuf;

pub struct CargoMetadataParameters<'a> {
    pub krates: &'a Krates,
    pub metadata: &'a Metadata,
}

pub trait DepsNotReplaced {
    fn deps_not_replaced(
        &self,
        package_id: cargo_metadata::PackageId,
    ) -> Vec<(
        cargo_metadata::PackageId,
        HashSet<cargo_metadata::Dependency>,
    )>;
}

pub trait GetLicenceFromCargoMetadataPackageId {
    fn get_licence_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<String>;
}

pub trait GetPackageNameFromCargoMetadataPackageId {
    fn get_package_name_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<String>;
}

pub trait GetPackageVersionFromCargoMetadataPackageId {
    fn get_package_version_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<cargo_metadata::Version>;
}

pub trait GetRepositoryFromCargoMetadataPackageId {
    fn get_repository_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> Option<String>;
}

pub trait GetRoot {
    fn get_root(&self) -> PathBuf;
}

pub trait MatchesIgnoringSource {
    fn matches_ignoring_source(
        &self,
        krates: &Krates,
        package_id: cargo_metadata::PackageId,
    ) -> bool;
}

pub trait QueryResolve {
    fn query_resolve(&self, query: &str) -> Option<cargo_metadata::PackageId>;
}

pub trait Replacement {
    fn replace(
        &self,
        cargo_metadata_parameters: &CargoMetadataParameters,
        package_set: &PackageSet,
        resolve: &Resolve,
    ) -> cargo_metadata::PackageId;
}

pub trait ToCargoCoreDepKind {
    fn to_cargo_core_dep_kind(&self) -> DepKind;
}

pub trait ToCargoGeigerPackageId {
    fn to_cargo_geiger_package_id(
        &self,
        krates: &Krates,
        package_set: &PackageSet,
    ) -> cargo_geiger_serde::PackageId;
}

pub trait ToCargoMetadataDependencyKind {
    fn to_cargo_metadata_dependency_kind(&self) -> DependencyKind;
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

pub trait ToPackage {
    fn to_package(
        &self,
        krates: &Krates,
        package_set: &PackageSet,
    ) -> Option<Package>;
}

pub trait ToPackageId {
    fn to_package_id(
        &self,
        krates: &Krates,
        package_set: &PackageSet,
    ) -> Option<PackageId>;
}
