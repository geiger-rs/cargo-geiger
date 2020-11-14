mod core;
mod krates;
mod metadata;

use ::krates::Krates;
use cargo::core::dependency::DepKind;
use cargo::core::manifest::ManifestMetadata;
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
        krates: &Krates,
        package_id: cargo_metadata::PackageId,
        package_set: &PackageSet,
        resolve: &Resolve,
    ) -> Vec<(
        cargo_metadata::PackageId,
        HashSet<cargo_metadata::Dependency>,
    )>;
}

pub trait GetManifestMetadataFromCargoMetadataPackageId {
    fn get_manifest_metadata_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
        package_set: &PackageSet,
    ) -> ManifestMetadata;
}

pub trait GetPackageNameFromCargoMetadataPackageId {
    fn get_package_name_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> String;
}

pub trait GetPackageVersionFromCargoMetadataPackageId {
    fn get_package_version_from_cargo_metadata_package_id(
        &self,
        package_id: &cargo_metadata::PackageId,
    ) -> cargo_metadata::Version;
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

pub trait ToCargoMetadataDependencyKind {
    fn to_cargo_metadata_dependency_kind(&self) -> DependencyKind;
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

pub trait ToPackage {
    fn to_package(&self, krates: &Krates, package_set: &PackageSet) -> Package;
}

pub trait ToPackageId {
    fn to_package_id(
        &self,
        krates: &Krates,
        package_set: &PackageSet,
    ) -> PackageId;
}
