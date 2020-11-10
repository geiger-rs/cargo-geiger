use crate::format::pattern::Pattern;
use crate::format::Chunk;

use crate::utils::{
    CargoMetadataParameters, GetPackageNameFromCargoMetadataPackageId,
    GetPackageVersionFromCargoMetadataPackageId,
};
use cargo::core::manifest::ManifestMetadata;
use std::fmt;

pub struct Display<'a> {
    pub cargo_metadata_parameters: &'a CargoMetadataParameters<'a>,
    pub pattern: &'a Pattern,
    pub package: &'a cargo_metadata::PackageId,
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
                        self.cargo_metadata_parameters
                            .krates
                            .get_package_name_from_cargo_metadata_package_id(
                                self.package
                            ),
                        self.cargo_metadata_parameters
                            .krates
                            .get_package_version_from_cargo_metadata_package_id(
                                self.package
                            )
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
    use cargo_metadata::{CargoOpt, MetadataCommand};
    use krates::Builder;
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
            "cargo-geiger 0.10.2"
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
        let metadata = MetadataCommand::new()
            .manifest_path("./Cargo.toml")
            .features(CargoOpt::AllFeatures)
            .exec()
            .unwrap();

        let krates = Builder::new()
            .build_with_metadata(metadata.clone(), |_| ())
            .unwrap();

        let package_id = metadata.root_package().unwrap().id.clone();

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
            cargo_metadata_parameters: &CargoMetadataParameters {
                krates: &krates,
                metadata: &metadata,
            },
            pattern: &input_pattern,
            package: &package_id,
            metadata: &manifest_metadata,
        };

        assert_eq!(format!("{}", display), expected_formatted_string);
    }
}
