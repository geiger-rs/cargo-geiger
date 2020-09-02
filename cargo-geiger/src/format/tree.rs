use crate::format::Charset;

use cargo::core::dependency::DepKind;
use cargo::core::PackageId;

/// A step towards decoupling some parts of the table-tree printing from the
/// dependency graph traversal.
pub enum TextTreeLine {
    /// A text line for a package
    Package { id: PackageId, tree_vines: String },
    /// There're extra dependencies comming and we should print a group header,
    /// eg. "[build-dependencies]".
    ExtraDepsGroup { kind: DepKind, tree_vines: String },
}

pub struct TreeSymbols {
    pub down: &'static str,
    pub tee: &'static str,
    pub ell: &'static str,
    pub right: &'static str,
}

pub fn get_tree_symbols(cs: Charset) -> TreeSymbols {
    match cs {
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
