use crate::PackageId;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    ops::{Add, AddAssign},
    path::PathBuf,
};

/// Package dependency information
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PackageInfo {
    pub id: PackageId,
    #[serde(serialize_with = "set_serde::serialize")]
    pub dependencies: HashSet<PackageId>,
    #[serde(serialize_with = "set_serde::serialize")]
    pub dev_dependencies: HashSet<PackageId>,
    #[serde(serialize_with = "set_serde::serialize")]
    pub build_dependencies: HashSet<PackageId>,
}

impl PackageInfo {
    pub fn new(id: PackageId) -> Self {
        PackageInfo {
            id,
            dependencies: Default::default(),
            dev_dependencies: Default::default(),
            build_dependencies: Default::default(),
        }
    }

    pub fn add_dependency(&mut self, dep: PackageId, kind: DependencyKind) {
        match kind {
            DependencyKind::Normal => self.dependencies.insert(dep),
            DependencyKind::Development => self.dev_dependencies.insert(dep),
            DependencyKind::Build => self.build_dependencies.insert(dep),
        };
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
    #[serde(with = "entry_serde")]
    pub packages: HashMap<PackageId, QuickReportEntry>,
    /// Packages that were not scanned successfully
    #[serde(serialize_with = "set_serde::serialize")]
    pub packages_without_metrics: HashSet<PackageId>,
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
    #[serde(with = "entry_serde")]
    pub packages: HashMap<PackageId, ReportEntry>,
    #[serde(serialize_with = "set_serde::serialize")]
    pub packages_without_metrics: HashSet<PackageId>,
    #[serde(serialize_with = "set_serde::serialize")]
    pub used_but_not_scanned_files: HashSet<PathBuf>,
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

trait Entry {
    fn package_id(&self) -> &PackageId;
}

impl Entry for ReportEntry {
    fn package_id(&self) -> &PackageId {
        &self.package.id
    }
}

impl Entry for QuickReportEntry {
    fn package_id(&self) -> &PackageId {
        &self.package.id
    }
}

mod entry_serde {
    use crate::PackageId;
    use serde::{
        ser::SerializeSeq,
        Deserialize, Deserializer, Serialize, Serializer,
    };
    use std::{
        collections::HashMap,
        fmt,
        marker::PhantomData,
    };

    pub(super) fn serialize<T, S>(
        map: &HashMap<PackageId, T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        T: Serialize + super::Entry,
        S: Serializer,
    {
        let mut values = map.values().collect::<Vec<_>>();
        values.sort_by(|a, b| a.package_id().cmp(b.package_id()));
        let mut seq = serializer.serialize_seq(Some(values.len()))?;
        for value in values {
            seq.serialize_element(value)?;
        }
        seq.end()
    }

    pub(super) fn deserialize<'de, T, D>(deserializer: D) -> Result<HashMap<PackageId, T>, D::Error>
    where
        T: Deserialize<'de> + super::Entry,
        D: Deserializer<'de>,
    {
        struct Visitor<U>(PhantomData<fn() -> U>);

        impl<'d, U> serde::de::Visitor<'d> for Visitor<U>
        where
            U: Deserialize<'d> + super::Entry,
        {
            type Value = HashMap<PackageId, U>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a sequence")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'d>,
            {
                let mut map = HashMap::new();
                while let Some(item) = seq.next_element::<U>()? {
                    map.insert(item.package_id().clone(), item);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(Visitor(PhantomData))
    }
}

mod set_serde {
    use serde::{
        ser::SerializeSeq,
        Serialize, Serializer,
    };
    use std::collections::HashSet;

    pub(super) fn serialize<T, S>(
        set: &HashSet<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        T: Serialize + Ord,
        S: Serializer,
    {
        let mut values = set.iter().collect::<Vec<_>>();
        values.sort_by(|a, b| a.cmp(b));
        let mut seq = serializer.serialize_seq(Some(values.len()))?;
        for value in values {
            seq.serialize_element(value)?;
        }
        seq.end()
    }
}
