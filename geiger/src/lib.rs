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

#[cfg(test)]
mod lib_tests {
    use super::*;
    use rstest::*;
    use syn::{File, ItemFn, MetaList};

    #[rstest(
        input_file_str,
        expected_file_forbids_unsafe,
        case(
            "#![forbid(unsafe_code)]
             #![deny(warnings)]
             mod a {
                fn b() {3}
             }",
            true,
        ),
        case(
            "#![deny(warnings)]
             mod a {
                fn b() {3}
             }",
            false,
        )
    )]
    fn file_forbids_unsafe_test(
        input_file_str: &str,
        expected_file_forbids_unsafe: bool,
    ) {
        let input_file: File = syn::parse_str(input_file_str).unwrap();

        assert_eq!(
            file_forbids_unsafe(&input_file),
            expected_file_forbids_unsafe,
        )
    }

    #[rstest(
        input_item_fn_str,
        expected_is_test_fn,
        case("#[test] fn a() { 3 }", true,),
        case("fn a() { 3 }", false,),
        case("#[rstest] fn a() { 3 }", false,)
    )]
    fn is_test_fn_test(input_item_fn_str: &str, expected_is_test_fn: bool) {
        let input_item_fn: ItemFn = syn::parse_str(input_item_fn_str).unwrap();

        assert_eq!(is_test_fn(&input_item_fn), expected_is_test_fn);
    }

    #[rstest(
        input_item_fn_str,
        expected_has_unsafe_attributes,
        case(
            "#[no_mangle]
             pub extern \"C\" fn hello_from_rust() {
                 println!(\"Hello from Rust!\");
             }",
            true
        ),
        case(
            "#[export_name = \"exported_symbol_name\"]
             pub fn name_in_rust() {
                 println!(\"Hello from Rust!\");
             }
            ",
            true
        ),
        case("fn a() { 3 }", false),
        case("#[test] fn a() { 3 }", false)
    )]
    fn has_unsafe_attributes_test(
        input_item_fn_str: &str,
        expected_has_unsafe_attributes: bool,
    ) {
        let input_item_fn: ItemFn = syn::parse_str(input_item_fn_str).unwrap();

        assert_eq!(
            has_unsafe_attributes(&input_item_fn),
            expected_has_unsafe_attributes
        )
    }

    #[rstest(
        input_item_mod_str,
        expected_is_test_mod,
        case("#[cfg(test)] mod a { fn b() {3} }", true,),
        case("mod a { fn b() {3} }", false,)
    )]
    fn is_test_mod_test(input_item_mod_str: &str, expected_is_test_mod: bool) {
        let input_mod: ItemMod = syn::parse_str(input_item_mod_str).unwrap();

        assert_eq!(is_test_mod(&input_mod), expected_is_test_mod,);
    }

    #[rstest(
        input_meta_list_str,
        expected_meta_list_is_cfg_test,
        case("cfg(test)", true,),
        case("cfg(feature)", false,),
        case("derive(Debug, Eq, PartialEq)", false,)
    )]
    fn meta_list_is_cfg_test_test(
        input_meta_list_str: &str,
        expected_meta_list_is_cfg_test: bool,
    ) {
        let input_meta_list: MetaList =
            syn::parse_str(input_meta_list_str).unwrap();

        assert_eq!(
            meta_list_is_cfg_test(&input_meta_list),
            expected_meta_list_is_cfg_test,
        )
    }
}
