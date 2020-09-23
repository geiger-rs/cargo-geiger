use cargo::core::PackageId;
use geiger::CounterBlock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SafetyReport {
    pub packages: Vec<ReportEntry>,
    pub used_but_not_scanned_files: Vec<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReportEntry {
    pub package: PackageInfo,
    pub unsafety: UnsafeInfo,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PackageInfo {
    pub id: PackageId,
    pub dependencies: Vec<PackageId>,
    pub dev_dependencies: Vec<PackageId>,
    pub build_dependencies: Vec<PackageId>,
}

impl PackageInfo {
    pub fn new(id: PackageId) -> Self {
        PackageInfo {
            id,
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            build_dependencies: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UnsafeInfo {
    pub used: CounterBlock,
    pub unused: CounterBlock,
    pub forbids_unsafe: bool,
}
