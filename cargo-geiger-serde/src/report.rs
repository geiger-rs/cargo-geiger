use crate::PackageId;
use serde::{Deserialize, Serialize};
use std::{
    ops::{Add, AddAssign},
    path::PathBuf,
};

/// Package dependency information
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

    pub fn push_dependency(&mut self, dep: PackageId, kind: DependencyKind) {
        match kind {
            DependencyKind::Normal => self.dependencies.push(dep),
            DependencyKind::Development => self.dev_dependencies.push(dep),
            DependencyKind::Build => self.build_dependencies.push(dep),
        }
    }
}

/// Entry of the report generated from scanning for packages that forbid the use of `unsafe`
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct QuickReportEntry {
    pub package: PackageInfo,
    /// Whether this package forbids the use of `unsafe`
    pub forbids_unsafe: bool,
}

/// Report generated from scanning for packages that forbid the use of `unsafe`
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct QuickSafetyReport {
    /// Packages that were scanned successfully
    pub packages: Vec<QuickReportEntry>,
    /// Packages that were not scanned successfully
    pub packages_without_metrics: Vec<PackageId>,
}

/// Entry of the report generated from scanning for the use of `unsafe`
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReportEntry {
    pub package: PackageInfo,
    /// Unsafety scan results
    pub unsafety: UnsafeInfo,
}

/// Report generated from scanning for the use of `unsafe`
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SafetyReport {
    pub packages: Vec<ReportEntry>,
    pub packages_without_metrics: Vec<PackageId>,
    pub used_but_not_scanned_files: Vec<PathBuf>,
}

/// Unsafety usage in a package
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct UnsafeInfo {
    /// Unsafe usage statistics for code used by the project
    pub used: CounterBlock,
    /// Unsafe usage statistics for code not used by the project
    pub unused: CounterBlock,
    /// Whether this package forbids the use of `unsafe`
    pub forbids_unsafe: bool,
}

/// Kind of dependency for a package
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum DependencyKind {
    /// Dependency in the `[dependencies]` section of `Cargo.toml`
    Normal,
    /// Dependency in the `[dev-dependencies]` section of `Cargo.toml`
    Development,
    /// Dependency in the `[build-dependencies]` section of `Cargo.toml`
    Build,
}

/// Statistics about the use of `unsafe`
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Count {
    /// Number of safe items
    pub safe: u64,
    /// Number of unsafe items
    pub unsafe_: u64,
}

impl Count {
    /// Increments the safe or unsafe counter by 1
    pub fn count(&mut self, is_unsafe: bool) {
        if is_unsafe {
            self.unsafe_ += 1;
        } else {
            self.safe += 1;
        }
    }
}

impl Add for Count {
    type Output = Count;

    fn add(self, other: Count) -> Count {
        Count {
            safe: self.safe + other.safe,
            unsafe_: self.unsafe_ + other.unsafe_,
        }
    }
}

impl AddAssign for Count {
    fn add_assign(&mut self, rhs: Count) {
        *self = self.clone() + rhs;
    }
}

/// Unsafe usage metrics collection.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CounterBlock {
    pub functions: Count,
    pub exprs: Count,
    pub item_impls: Count,
    pub item_traits: Count,
    pub methods: Count,
}

impl CounterBlock {
    pub fn has_unsafe(&self) -> bool {
        self.functions.unsafe_ > 0
            || self.exprs.unsafe_ > 0
            || self.item_impls.unsafe_ > 0
            || self.item_traits.unsafe_ > 0
            || self.methods.unsafe_ > 0
    }
}

impl Add for CounterBlock {
    type Output = CounterBlock;

    fn add(self, other: CounterBlock) -> CounterBlock {
        CounterBlock {
            functions: self.functions + other.functions,
            exprs: self.exprs + other.exprs,
            item_impls: self.item_impls + other.item_impls,
            item_traits: self.item_traits + other.item_traits,
            methods: self.methods + other.methods,
        }
    }
}

impl AddAssign for CounterBlock {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.clone() + rhs;
    }
}
