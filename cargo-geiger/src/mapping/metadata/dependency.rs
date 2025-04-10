use krates::cm::Dependency;
use krates::semver::VersionReq;

pub trait GetDependencyInformation {
    fn get_dependency_name(&self) -> String;
    fn get_dependency_version_req(&self) -> VersionReq;
}

impl GetDependencyInformation for Dependency {
    fn get_dependency_name(&self) -> String {
        self.name.clone()
    }
    fn get_dependency_version_req(&self) -> VersionReq {
        self.req.clone()
    }
}
