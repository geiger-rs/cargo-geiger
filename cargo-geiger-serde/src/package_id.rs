use crate::Source;
use semver::Version;
use serde::{Deserialize, Serialize};

/// Identifies a package in the dependency tree
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PackageId {
    /// Package name
    pub name: String,
    /// Package version
    pub version: Version,
    /// Package source (e.g. repository, crate registry)
    pub source: Source,
}
