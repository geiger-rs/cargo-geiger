use super::{ToCargoGeigerSource, ToCargoMetadataPackage};

use cargo_metadata::Metadata;
use url::Url;

impl ToCargoGeigerSource for cargo_metadata::PackageId {
    fn to_cargo_geiger_source(
        &self,
        metadata: &Metadata,
    ) -> cargo_geiger_serde::Source {
        let package = self.to_cargo_metadata_package(metadata).unwrap();

        match package.source {
            Some(source) => handle_source_repr(&source.repr),
            None => handle_path_source(self),
        }
    }
}

fn handle_source_repr(source_repr: &str) -> cargo_geiger_serde::Source {
    let mut source_repr_vec = source_repr.split('+').collect::<Vec<&str>>();

    let source_type = source_repr_vec[0];

    match source_type {
        "registry" => {
            cargo_geiger_serde::Source::Registry {
                // It looks like cargo metadata drops this information
                name: String::from("crates.io"),
                url: Url::parse(source_repr_vec.pop().unwrap()).unwrap(),
            }
        }
        "git" => {
            let raw_git_representation = source_repr_vec.pop().unwrap();
            let raw_git_url = Url::parse(raw_git_representation).unwrap();
            let git_url_without_query = format!(
                "{}://{}{}",
                raw_git_url.scheme(),
                raw_git_url.host_str().unwrap(),
                raw_git_url.path()
            );
            let revision = raw_git_url
                .query_pairs()
                .find(|(query_key, _)| query_key == "rev")
                .unwrap()
                .1;

            cargo_geiger_serde::Source::Git {
                url: Url::parse(&git_url_without_query).unwrap(),
                rev: String::from(revision),
            }
        }
        _ => panic!("Unrecognised source type: {}", source_type),
    }
}

fn handle_path_source(
    package_id: &cargo_metadata::PackageId,
) -> cargo_geiger_serde::Source {
    let raw_repr = package_id.repr.clone();
    let raw_path_repr = raw_repr[1..raw_repr.len() - 1]
        .split("+file://")
        .skip(1)
        .collect::<Vec<&str>>()
        .pop()
        .unwrap();

    let source_url: Url;
    if cfg!(windows) {
        source_url = Url::from_file_path(&raw_path_repr[1..]).unwrap();
    } else {
        source_url = Url::from_file_path(raw_path_repr).unwrap();
    }

    cargo_geiger_serde::Source::Path(source_url)
}

#[cfg(test)]
mod geiger_tests {
    use super::*;

    use rstest::*;
    use url::Url;

    #[rstest(
        input_source_repr,
        expected_source,
        case(
            "registry+https://github.com/rust-lang/crates.io-index",
            cargo_geiger_serde::Source::Registry {
                name: String::from("crates.io"),
                url: Url::parse("https://github.com/rust-lang/crates.io-index").unwrap()
            }
        ),
        case(
            "git+https://github.com/rust-itertools/itertools.git?rev=8761fbefb3b209",
            cargo_geiger_serde::Source::Git {
                url: Url::parse("https://github.com/rust-itertools/itertools.git").unwrap(),
                rev: String::from("8761fbefb3b209")
            }
        )
    )]
    fn handle_source_repr_test(
        input_source_repr: &str,
        expected_source: cargo_geiger_serde::Source,
    ) {
        let source = handle_source_repr(input_source_repr);
        assert_eq!(source, expected_source);
    }

    #[rstest]
    fn handle_path_source_test() {
        if !cfg!(windows) {
            let package_id = cargo_metadata::PackageId {
                repr: String::from("(path+file:///cargo_geiger/test_crates/test1_package_with_no_deps)"),
            };

            let expected_source = cargo_geiger_serde::Source::Path(
                Url::from_file_path(
                    "/cargo_geiger/test_crates/test1_package_with_no_deps",
                )
                .unwrap(),
            );

            let source = handle_path_source(&package_id);
            assert_eq!(source, expected_source);
        }
    }
}
