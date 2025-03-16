use crate::mapping::krates::GetNodeForKid;
use crate::mapping::GetPackageIdInformation;
use cargo_metadata::semver::Version;
use cargo_metadata::{Metadata, Package, PackageId};

impl GetPackageIdInformation for PackageId {
    fn get_package_id_licence<T: GetNodeForKid>(
        &self,
        krates: &T,
    ) -> Option<String> {
        krates
            .get_node_for_kid(self)
            .and_then(|package| match package {
                krates::Node::Krate { krate, .. } => krate.license.clone(),
                _ => None,
            })
    }

    fn get_package_id_name_and_version<T: GetNodeForKid>(
        &self,
        krates: &T,
    ) -> Option<(String, Version)> {
        match krates.get_node_for_kid(self) {
            Some(krates::Node::Krate { krate, .. }) => {
                Some((krate.name.clone(), krate.version.clone()))
            }
            _ => None,
        }
    }

    fn get_package_id_repository<T: GetNodeForKid>(
        &self,
        krates: &T,
    ) -> Option<String> {
        krates
            .get_node_for_kid(self)
            .and_then(|package| match package {
                krates::Node::Krate { krate, .. } => krate.repository.clone(),
                _ => None,
            })
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
