use cargo_metadata::{Metadata, PackageId, Package};

pub trait GetPackageIdRepr {
    fn get_package_id_repr(&self) -> String;
}

impl GetPackageIdRepr for PackageId {
    fn get_package_id_repr(&self) -> String {
        self.repr.clone()
    }
}

pub trait ToCargoMetadataPackage {
    fn to_cargo_metadata_package(
        &self,
        metadata: &Metadata,
    ) -> Option<Package>;
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