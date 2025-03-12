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
use syn::{AttrStyle, ItemFn, ItemMod};

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
    f.attrs.iter().any(|attr| {
        // https://docs.rs/syn/latest/syn/meta/struct.ParseNestedMeta.html#example
        let mut is_forbid_unsafe_code = false;
        if matches!(attr.style, AttrStyle::Inner(_)) {
            // Parses `#!`.
            if attr.path().is_ident("forbid") {
                // Parses `forbid`.
                let _ = attr.parse_nested_meta(|meta| {
                    // Parses `(`.
                    if meta.path.is_ident("unsafe_code") {
                        if meta.value().is_err() {
                            is_forbid_unsafe_code = true;
                        }
                    }
                    Ok(())
                });
            }
        }
        is_forbid_unsafe_code
    })
}

fn is_test_fn(item_fn: &ItemFn) -> bool {
    item_fn
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("test"))
}

fn has_unsafe_attributes(item_fn: &ItemFn) -> bool {
    item_fn.attrs.iter().any(|attr| {
        if attr.path().is_ident("no_mangle") {
            return true;
        }
        if attr.path().is_ident("export_name") {
            return true;
        }
        false
    })
}

/// Will return true for #[cfg(test)] decorated modules.
///
/// This function is a somewhat of a hack and will probably misinterpret more
/// advanced cfg expressions. A better way to do this would be to let rustc emit
/// every single source file path and span within each source file and use that
/// as a general filter for included code.
/// TODO: Investigate if the needed information can be emitted by rustc today.
fn is_test_mod(item: &ItemMod) -> bool {
    item.attrs.iter().any(|attr| {
        let mut found_cfg_test = false;
        if attr.path().is_ident("cfg") {
            let _ = attr.parse_nested_meta(|meta| {
                // Parse `(`.
                if meta.path.is_ident("test") {
                    found_cfg_test = true;
                }
                Ok(())
            });
        }
        found_cfg_test
    })
}
