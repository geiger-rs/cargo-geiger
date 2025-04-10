use crate::mapping::krates_mapping::GetPackage;
use crate::mapping::GetPackageIdInformation;
use krates::cm::{Metadata, Package, PackageId};
use krates::semver::Version;

impl GetPackageIdInformation for PackageId {
    fn get_package_id_licence<T: GetPackage>(
        &self,
        krates: &T,
    ) -> Option<String> {
        krates
            .get_package(self)
            .and_then(|package| package.license.clone())
    }

    fn get_package_id_name_and_version<T: GetPackage>(
        &self,
        krates: &T,
    ) -> Option<(String, Version)> {
        krates
            .get_package(self)
            .map(|package| (package.name.clone(), package.version.clone()))
    }

    fn get_package_id_repository<T: GetPackage>(
        &self,
        krates: &T,
    ) -> Option<String> {
        krates
            .get_package(self)
            .and_then(|package| package.repository.clone())
    }
}

pub trait GetPackageIdRepr {
    fn get_package_id_repr(&self) -> String;
}

impl GetPackageIdRepr for PackageId {
    fn get_package_id_repr(&self) -> String {
        self.repr.clone()
    }
}

pub trait ToCargoMetadataPackage {
    fn to_cargo_metadata_package(&self, metadata: &Metadata)
        -> Option<Package>;
}

impl ToCargoMetadataPackage for PackageId {
    fn to_cargo_metadata_package(
        &self,
        metadata: &Metadata,
    ) -> Option<Package> {
        metadata
            .packages
            .iter()
            .filter(|p| p.id == *self)
            .cloned()
            .collect::<Vec<Package>>()
            .pop()
    }
}
