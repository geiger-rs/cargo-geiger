use crate::format::Charset;

use cargo::core::dependency::DepKind;
use cargo::core::PackageId;

/// A step towards decoupling some parts of the table-tree printing from the
/// dependency graph traversal.
pub enum TextTreeLine {
    /// A text line for a package
    Package { id: PackageId, tree_vines: String },
    /// There are extra dependencies coming and we should print a group header,
    /// eg. "[build-dependencies]".
    ExtraDepsGroup { kind: DepKind, tree_vines: String },
}

#[derive(Debug, PartialEq)]
pub struct TreeSymbols {
    pub down: &'static str,
    pub tee: &'static str,
    pub ell: &'static str,
    pub right: &'static str,
}

pub fn get_tree_symbols(charset: Charset) -> TreeSymbols {
    match charset {
        Charset::Utf8 => UTF8_TREE_SYMBOLS,
        Charset::Ascii => ASCII_TREE_SYMBOLS,
    }
}

const ASCII_TREE_SYMBOLS: TreeSymbols = TreeSymbols {
    down: "|",
    tee: "|",
    ell: "`",
    right: "-",
};

const UTF8_TREE_SYMBOLS: TreeSymbols = TreeSymbols {
    down: "│",
    tee: "├",
    ell: "└",
    right: "─",
};

#[cfg(test)]
mod tree_tests {
    use super::*;

    #[test]
    fn get_tree_symbols_test() {
        assert_eq!(get_tree_symbols(Charset::Utf8), UTF8_TREE_SYMBOLS);
    }
}
