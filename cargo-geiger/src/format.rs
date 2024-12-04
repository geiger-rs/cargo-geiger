pub mod emoji_symbols;
pub mod pattern;
pub mod print_config;
pub mod table;

mod display;
mod parse;

use cargo_metadata::DependencyKind;
use std::fmt;
use std::str::{self, FromStr};
use strum_macros::EnumIter;

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub enum Charset {
    #[default]
    Ascii,
    GitHubMarkdown,
    Utf8,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Chunk {
    License,
    Package,
    Raw(String),
    Repository,
}

impl FromStr for Charset {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Charset, &'static str> {
        let comparison_string = String::from(s).to_lowercase();
        match comparison_string.as_str() {
            "ascii" => Ok(Charset::Ascii),
            "githubmarkdown" => Ok(Charset::GitHubMarkdown),
            "utf8" => Ok(Charset::Utf8),
            _ => Err("invalid charset"),
        }
    }
}

#[derive(Debug, Clone, EnumIter, Eq, PartialEq)]
pub enum CrateDetectionStatus {
    NoneDetectedForbidsUnsafe,
    NoneDetectedAllowsUnsafe,
    UnsafeDetected,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RawChunk<'a> {
    Argument(&'a str),
    Error(&'static str),
    Text(&'a str),
}

#[derive(Clone, Copy)]
pub enum SymbolKind {
    Lock = 0,
    QuestionMark = 1,
    Rads = 2,
}

#[derive(Debug)]
pub struct FormatError {
    pub message: String,
}

impl std::error::Error for FormatError {}

/// Forward Display to Debug, probably good enough for programmer facing error
/// messages.
impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

pub fn get_kind_group_name(dep_kind: DependencyKind) -> Option<&'static str> {
    match dep_kind {
        DependencyKind::Build => Some("[build-dependencies]"),
        DependencyKind::Development => Some("[dev-dependencies]"),
        DependencyKind::Normal => None,
        _ => panic!("Unrecognised Dependency Kind"),
    }
}

#[cfg(test)]
mod format_tests {
    use super::*;

    use rstest::*;

    #[rstest(
        input_string,
        expected_enum_result,
        case("ascii", Ok(Charset::Ascii)),
        case("githubmarkdown", Ok(Charset::GitHubMarkdown)),
        case("utf8", Ok(Charset::Utf8)),
        case("UTF8", Ok(Charset::Utf8)),
        case("invalid_str", Err("invalid charset"))
    )]
    fn charset_from_str_test(
        input_string: &str,
        expected_enum_result: Result<Charset, &'static str>,
    ) {
        assert_eq!(Charset::from_str(input_string), expected_enum_result);
    }

    #[rstest]
    fn get_kind_group_name_test() {
        assert_eq!(
            get_kind_group_name(DependencyKind::Build),
            Some("[build-dependencies]")
        );

        assert_eq!(
            get_kind_group_name(DependencyKind::Development),
            Some("[dev-dependencies]")
        );

        assert_eq!(get_kind_group_name(DependencyKind::Normal), None);
    }
}
