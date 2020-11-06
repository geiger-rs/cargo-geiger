pub mod emoji_symbols;
pub mod pattern;
pub mod print_config;
pub mod table;

mod display;
mod parse;

use cargo::core::dependency::DepKind;
use std::fmt;
use std::str::{self, FromStr};
use strum_macros::EnumIter;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Charset {
    Ascii,
    Utf8,
}

impl Default for Charset {
    fn default() -> Self {
        Charset::Ascii
    }
}

#[derive(Debug, PartialEq)]
pub enum Chunk {
    License,
    Package,
    Raw(String),
    Repository,
}

impl FromStr for Charset {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Charset, &'static str> {
        match s {
            "ascii" => Ok(Charset::Ascii),
            "utf8" => Ok(Charset::Utf8),
            _ => Err("invalid charset"),
        }
    }
}

#[derive(Debug, Clone, EnumIter, PartialEq)]
pub enum CrateDetectionStatus {
    NoneDetectedForbidsUnsafe,
    NoneDetectedAllowsUnsafe,
    UnsafeDetected,
}

#[derive(Debug, PartialEq)]
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

pub fn get_kind_group_name(dep_kind: DepKind) -> Option<&'static str> {
    match dep_kind {
        DepKind::Build => Some("[build-dependencies]"),
        DepKind::Development => Some("[dev-dependencies]"),
        DepKind::Normal => None,
    }
}

#[cfg(test)]
mod format_tests {
    use super::*;

    use rstest::*;

    #[rstest]
    fn charset_from_str_test() {
        assert_eq!(Charset::from_str("ascii"), Ok(Charset::Ascii));
        assert_eq!(Charset::from_str("utf8"), Ok(Charset::Utf8));
        assert_eq!(Charset::from_str("invalid_str"), Err("invalid charset"));
    }

    #[rstest]
    fn get_kind_group_name_test() {
        assert_eq!(
            get_kind_group_name(DepKind::Build),
            Some("[build-dependencies]")
        );

        assert_eq!(
            get_kind_group_name(DepKind::Development),
            Some("[dev-dependencies]")
        );

        assert_eq!(get_kind_group_name(DepKind::Normal), None);
    }
}
