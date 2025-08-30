pub mod emoji_symbols;
pub mod pattern;
pub mod print_config;
pub mod table;

mod display;
mod parse;

use krates::cm::DependencyKind;
use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Eq, PartialEq)]
pub enum Chunk {
    License,
    Package,
    Raw(String),
    Repository,
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
#[allow(dead_code)]
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
