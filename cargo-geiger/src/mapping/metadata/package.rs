use cargo_metadata::{Package, Version};
use std::path::Path;

pub trait GetPackageName {
    fn get_package_name(&self) -> String;
}

pub trait GetPackageParent {
    fn get_package_parent(&self) -> Option<&Path>;
}

pub trait GetPackageVersion {
    fn get_package_version(&self) -> Version;
}

impl GetPackageName for Package {
    fn get_package_name(&self) -> String {
        self.name.clone()
    }
}

impl GetPackageParent for Package {
    fn get_package_parent(&self) -> Option<&Path> {
        self.manifest_path.parent()
    }
}

impl GetPackageVersion for Package {
    fn get_package_version(&self) -> Version {
        self.version.clone()
    }
}