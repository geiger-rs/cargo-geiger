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
                Chunk::License => {
                    if let Some(ref license) = self.metadata.license {
                        (write!(fmt, "{}", license))?
                    }
                }
                Chunk::Package => {
                    (write!(
                        fmt,
                        "{} {}",
                        self.package.name(),
                        self.package.version()
                    ))?
                }
                Chunk::Raw(ref s) => (fmt.write_str(s))?,
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

#[cfg(test)]
pub mod display_tests {
    use super::*;

    use crate::format::pattern::Pattern;
    use crate::format::Chunk;

    use cargo::core::manifest::ManifestMetadata;
    use cargo::core::{PackageId, SourceId};
    use cargo::util::ToSemver;
    use rstest::*;

    #[rstest(
        input_pattern,
        expected_formatted_string,
        case(
            Pattern(vec![Chunk::License]),
            "licence_string"
        ),
        case(
            Pattern(vec![Chunk::Package]),
            "package_name 1.2.3"
        ),
        case(
            Pattern(vec![Chunk::Raw(String::from("chunk_value"))]),
            "chunk_value"
        ),
        case(
            Pattern(vec![Chunk::Repository]),
            "repository_string"
        )
    )]
    fn display_format_fmt_test(
        input_pattern: Pattern,
        expected_formatted_string: &str,
    ) {
        let package_id = PackageId::new(
            "package_name",
            "1.2.3".to_semver().unwrap(),
            SourceId::from_url(
                "git+https://github.com/rust-secure-code/cargo-geiger",
            )
            .unwrap(),
        )
        .unwrap();

        let manifest_metadata = ManifestMetadata {
            authors: vec![],
            keywords: vec![],
            categories: vec![],
            license: Some(String::from("licence_string")),
            license_file: None,
            description: None,
            readme: None,
            homepage: None,
            repository: Some(String::from("repository_string")),
            documentation: None,
            badges: Default::default(),
            links: None,
        };

        let display = Display {
            pattern: &input_pattern,
            package: &package_id,
            metadata: &manifest_metadata,
        };

        assert_eq!(format!("{}", display), expected_formatted_string);
    }
}
