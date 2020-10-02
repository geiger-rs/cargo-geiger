use crate::format::pattern::Pattern;
use crate::format::Chunk;

use cargo::core::manifest::ManifestMetadata;
use cargo::core::PackageId;
use std::fmt;

pub struct Display<'a> {
    pub pattern: &'a Pattern,
    pub package: &'a PackageId,
    pub metadata: &'a ManifestMetadata,
}

impl<'a> fmt::Display for Display<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for chunk in &self.pattern.0 {
            match *chunk {
                Chunk::Raw(ref s) => (fmt.write_str(s))?,
                Chunk::Package => {
                    (write!(
                        fmt,
                        "{} {}",
                        self.package.name(),
                        self.package.version()
                    ))?
                }
                Chunk::License => {
                    if let Some(ref license) = self.metadata.license {
                        (write!(fmt, "{}", license))?
                    }
                }
                Chunk::Repository => {
                    if let Some(ref repository) = self.metadata.repository {
                        (write!(fmt, "{}", repository))?
                    }
                }
            }
        }

        Ok(())
    }
}
