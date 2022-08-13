//! geiger ☢
//! ========
//!
//! This crate provides some library parts used by `cargo-geiger` that are decoupled
//! from `cargo`.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub mod find;
pub use find::*; // preserve APIs

mod geiger_syn_visitor;

use cargo_geiger_serde::CounterBlock;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::string::FromUtf8Error;
use syn::{ItemFn, ItemMod};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncludeTests {
    Yes,
    No,
}

/// Scan result for a single `.rs` file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RsFileMetrics {
    /// Metrics storage.
    pub counters: CounterBlock,

    /// This file is decorated with `#![forbid(unsafe_code)]`
    pub forbids_unsafe: bool,
}

#[derive(Debug)]
pub enum ScanFileError {
    Io(io::Error, PathBuf),
    Utf8(FromUtf8Error, PathBuf),
    Syn(syn::Error, PathBuf),
}

impl Error for ScanFileError {}

/// Forward Display to Debug, probably good enough for
/// programmer facing error messages.
impl fmt::Display for ScanFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

fn file_forbids_unsafe(f: &syn::File) -> bool {
    use syn::AttrStyle;
    use syn::Meta;
    use syn::MetaList;
    use syn::NestedMeta;
    f.attrs
        .iter()
        .filter(|a| matches!(a.style, AttrStyle::Inner(_)))
        .filter_map(|a| a.parse_meta().ok())
        .filter(|meta| match meta {
            Meta::List(MetaList {
                path,
                paren_token: _paren,
                nested,
            }) => {
                if !path.is_ident("forbid") {
                    return false;
                }
                nested.iter().any(|n| match n {
                    NestedMeta::Meta(Meta::Path(p)) => {
                        p.is_ident("unsafe_code")
                    }
                    _ => false,
                })
            }
            _ => false,
        })
        .count()
        > 0
}

fn is_test_fn(item_fn: &ItemFn) -> bool {
    use syn::Attribute;
    item_fn
        .attrs
        .iter()
        .flat_map(Attribute::parse_meta)
        .any(|m| meta_contains_ident(&m, "test"))
}

fn has_unsafe_attributes(item_fn: &ItemFn) -> bool {
    use syn::Attribute;
    item_fn
        .attrs
        .iter()
        .flat_map(Attribute::parse_meta)
        .any(|m| {
            meta_contains_ident(&m, "no_mangle")
                || meta_contains_attribute(&m, "export_name")
        })
}

/// Will return true for #[cfg(test)] decorated modules.
///
/// This function is a somewhat of a hack and will probably misinterpret more
/// advanced cfg expressions. A better way to do this would be to let rustc emit
/// every single source file path and span within each source file and use that
/// as a general filter for included code.
/// TODO: Investigate if the needed information can be emitted by rustc today.
fn is_test_mod(i: &ItemMod) -> bool {
    use syn::Attribute;
    use syn::Meta;
    i.attrs
        .iter()
        .flat_map(Attribute::parse_meta)
        .any(|m| match m {
            Meta::List(ml) => meta_list_is_cfg_test(&ml),
            _ => false,
        })
}

fn meta_contains_ident(m: &syn::Meta, ident: &str) -> bool {
    use syn::Meta;
    match m {
        Meta::Path(p) => p.is_ident(ident),
        _ => false,
    }
}

fn meta_contains_attribute(m: &syn::Meta, ident: &str) -> bool {
    use syn::Meta;
    match m {
        Meta::NameValue(nv) => nv.path.is_ident(ident),
        _ => false,
    }
}

// MetaList {
//     ident: Ident(
//         cfg
//     ),
//     paren_token: Paren,
//     nested: [
//         Meta(
//             Word(
//                 Ident(
//                     test
//                 )
//             )
//         )
//     ]
// }
fn meta_list_is_cfg_test(meta_list: &syn::MetaList) -> bool {
    use syn::NestedMeta;
    if !meta_list.path.is_ident("cfg") {
        return false;
    }
    meta_list.nested.iter().any(|n| match n {
        NestedMeta::Meta(meta) => meta_contains_ident(meta, "test"),
        _ => false,
    })
}
