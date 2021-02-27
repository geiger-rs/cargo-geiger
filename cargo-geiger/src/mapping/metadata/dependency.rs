use cargo_metadata::Dependency;
use krates::semver::VersionReq;

pub trait GetDependencyName {
    fn get_dependency_name(&self) -> String;
}

impl GetDependencyName for Dependency {
    fn get_dependency_name(&self) -> String {
        self.name.clone()
    }
}

pub trait GetDependencyRequirement {
    fn get_dependency_requirement(&self) -> VersionReq;
}

impl GetDependencyRequirement for Dependency {
    fn get_dependency_requirement(&self) -> VersionReq {
        self.req.clone()
    }
}