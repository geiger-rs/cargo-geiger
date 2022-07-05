use cargo_metadata::semver::Version;
use cargo_metadata::Package;
use std::path::Path;

use crate::mapping::GetPackageRoot;

pub trait GetPackageInformation {
    fn get_package_name(&self) -> String;

    fn get_package_parent(&self) -> Option<&Path>;

    fn get_package_version(&self) -> Version;
}

impl GetPackageInformation for Package {
    fn get_package_name(&self) -> String {
        self.name.clone()
    }

    fn get_package_parent(&self) -> Option<&Path> {
        self.manifest_path.parent().map(|p| p.as_ref())
    }

    fn get_package_version(&self) -> Version {
        self.version.clone()
    }
}

impl GetPackageRoot for Package {}
