//! geiger ☢
//! ========
//!
//! This crate provides some library parts used by `cargo-geiger` that are decoupled
//! from `cargo`.

#![forbid(unsafe_code)]
#![forbid(warnings)]

extern crate proc_macro2;
extern crate syn;
extern crate walkdir;

use self::walkdir::DirEntry;
use self::walkdir::WalkDir;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::Read;
use std::ops::Add;
use std::path::Path;
use std::path::PathBuf;
use std::string::FromUtf8Error;
use syn::{visit, Expr, ImplItemMethod, ItemFn, ItemImpl, ItemMod, ItemTrait};

#[derive(Debug)]
pub enum ScanFileError {
    Io(io::Error, PathBuf),
    Utf8(FromUtf8Error, PathBuf),
    Syn(syn::Error, PathBuf),
}

impl Error for ScanFileError {}

/// Forward Display to Debug. See the crate root documentation.
impl fmt::Display for ScanFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Default, Clone)]
pub struct Count {
    /// Number of safe items
    pub safe: u64,

    /// Number of unsafe items
    pub unsafe_: u64,
}

impl Count {
    fn count(&mut self, is_unsafe: bool) {
        if is_unsafe {
            self.unsafe_ += 1;
        } else {
            self.safe += 1;
        }
    }
}

impl Add for Count {
    type Output = Count;

    fn add(self, other: Count) -> Count {
        Count {
            safe: self.safe + other.safe,
            unsafe_: self.unsafe_ + other.unsafe_,
        }
    }
}

/// Unsafe usage metrics collection.
#[derive(Debug, Default, Clone)]
pub struct CounterBlock {
    pub functions: Count,
    pub exprs: Count,
    pub item_impls: Count,
    pub item_traits: Count,
    pub methods: Count,
}

impl CounterBlock {
    pub fn has_unsafe(&self) -> bool {
        self.functions.unsafe_ > 0
            || self.exprs.unsafe_ > 0
            || self.item_impls.unsafe_ > 0
            || self.item_traits.unsafe_ > 0
            || self.methods.unsafe_ > 0
    }
}

impl Add for CounterBlock {
    type Output = CounterBlock;

    fn add(self, other: CounterBlock) -> CounterBlock {
        CounterBlock {
            functions: self.functions + other.functions,
            exprs: self.exprs + other.exprs,
            item_impls: self.item_impls + other.item_impls,
            item_traits: self.item_traits + other.item_traits,
            methods: self.methods + other.methods,
        }
    }
}

/// Scan result for a single `.rs` file.
#[derive(Debug, Default)]
pub struct RsFileMetrics {
    /// Metrics storage.
    pub counters: CounterBlock,

    /// This file is decorated with `#![forbid(unsafe_code)]`
    pub forbids_unsafe: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum IncludeTests {
    Yes,
    No,
}

struct GeigerSynVisitor {
    /// Count unsafe usage inside tests
    include_tests: IncludeTests,

    /// The resulting data from a single file scan.
    metrics: RsFileMetrics,

    /// Used by the Visit trait implementation to track the traversal state.
    in_unsafe_block: bool,
}

impl GeigerSynVisitor {
    fn new(include_tests: IncludeTests) -> Self {
        GeigerSynVisitor {
            include_tests,
            metrics: Default::default(),
            in_unsafe_block: false,
        }
    }
}

/// Will return true for #[cfg(test)] decodated modules.
///
/// This function is a somewhat of a hack and will probably misinterpret more
/// advanced cfg expressions. A better way to do this would be to let rustc emit
/// every single source file path and span within each source file and use that
/// as a general filter for included code.
/// TODO: Investigate if the needed information can be emitted by rustc today.
fn is_test_mod(i: &ItemMod) -> bool {
    use syn::Meta;
    use syn::Attribute;
    i.attrs
        .iter()
        .flat_map(Attribute::interpret_meta)
        .any(|m| match m {
            Meta::List(ml) => meta_list_is_cfg_test(&ml),
            _ => false,
        })
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
fn meta_list_is_cfg_test(ml: &syn::MetaList) -> bool {
    use syn::NestedMeta;
    if ml.ident != "cfg" {
        return false;
    }
    ml.nested.iter().any(|n| match n {
        NestedMeta::Meta(meta) => meta_is_word_test(meta),
        _ => false,
    })
}

fn meta_is_word_test(m: &syn::Meta) -> bool {
    use syn::Meta;
    match m {
        Meta::Word(ident) => ident == "test",
        _ => false,
    }
}

fn is_test_fn(i: &ItemFn) -> bool {
    use syn::Attribute;
    i.attrs
        .iter()
        .flat_map(Attribute::interpret_meta)
        .any(|m| meta_is_word_test(&m))
}

fn file_forbids_unsafe(f: &syn::File) -> bool {
    use proc_macro2::{Ident, Span};
    use syn::AttrStyle;
    use syn::Meta;
    use syn::MetaList;
    use syn::NestedMeta;
    let forbid_ident = Ident::new("forbid", Span::call_site());
    let unsafe_code_ident = Ident::new("unsafe_code", Span::call_site());
    f.attrs
        .iter()
        .filter(|a| match a.style {
            AttrStyle::Inner(_) => true,
            _ => false,
        })
        .filter_map(|a| a.parse_meta().ok())
        .filter(|meta| match meta {
            Meta::List(MetaList {
                ident,
                paren_token: _paren,
                nested,
            }) => {
                if ident != &forbid_ident {
                    return false;
                }
                nested.iter().any(|n| match n {
                    NestedMeta::Meta(Meta::Word(word)) => {
                        word == &unsafe_code_ident
                    }
                    _ => false,
                })
            }
            _ => false,
        })
        .count()
        > 0
}

impl<'ast> visit::Visit<'ast> for GeigerSynVisitor {
    fn visit_file(&mut self, i: &'ast syn::File) {
        self.metrics.forbids_unsafe = file_forbids_unsafe(i);
        syn::visit::visit_file(self, i);
    }

    /// Free-standing functions
    fn visit_item_fn(&mut self, i: &ItemFn) {
        if IncludeTests::No == self.include_tests && is_test_fn(i) {
            return;
        }
        self.metrics.counters.functions.count(i.unsafety.is_some());
        visit::visit_item_fn(self, i);
    }

    fn visit_expr(&mut self, i: &Expr) {
        // Total number of expressions of any type
        match i {
            Expr::Unsafe(i) => {
                self.in_unsafe_block = true;
                visit::visit_expr_unsafe(self, i);
                self.in_unsafe_block = false;
            }
            Expr::Path(_) | Expr::Lit(_) => {
                // Do not count. The expression `f(x)` should count as one
                // expression, not three.
            }
            other => {
                // TODO: Print something pretty here or gather the data for later
                // printing.
                // if self.verbosity == Verbosity::Verbose && self.in_unsafe_block {
                //     println!("{:#?}", other);
                // }
                self.metrics.counters.exprs.count(self.in_unsafe_block);
                visit::visit_expr(self, other);
            }
        }
    }

    fn visit_item_mod(&mut self, i: &ItemMod) {
        if IncludeTests::No == self.include_tests && is_test_mod(i) {
            return;
        }
        visit::visit_item_mod(self, i);
    }

    fn visit_item_impl(&mut self, i: &ItemImpl) {
        // unsafe trait impl's
        self.metrics.counters.item_impls.count(i.unsafety.is_some());
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, i: &ItemTrait) {
        // Unsafe traits
        self.metrics
            .counters
            .item_traits
            .count(i.unsafety.is_some());
        visit::visit_item_trait(self, i);
    }

    fn visit_impl_item_method(&mut self, i: &ImplItemMethod) {
        self.metrics
            .counters
            .methods
            .count(i.sig.unsafety.is_some());
        visit::visit_impl_item_method(self, i);
    }

    // TODO: Visit macros.
    //
    // TODO: Figure out if there are other visit methods that should be
    // implemented here.
}

// NOTE: The same code exist in `cargo-geiger`, see the comment in that crate
// for more details.
fn is_file_with_ext(entry: &DirEntry, file_ext: &str) -> bool {
    if !entry.file_type().is_file() {
        return false;
    }
    let p = entry.path();
    let ext = match p.extension() {
        Some(e) => e,
        None => return false,
    };
    // to_string_lossy is ok since we only want to match against an ASCII
    // compatible extension and we do not keep the possibly lossy result
    // around.
    ext.to_string_lossy() == file_ext
}

/// TODO: Review this, should this be public? Hide this as private and export a
/// `pub fn find_unsafe_in_dir` instead(?).
/// Or require the caller to perform all directory walking?
pub fn find_rs_files_in_dir(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let walker = WalkDir::new(dir).into_iter();
    walker.filter_map(|entry| {
        let entry = entry.expect("walkdir error."); // TODO: Return result.
        if !is_file_with_ext(&entry, "rs") {
            return None;
        }
        Some(
            entry
                .path()
                .canonicalize()
                .expect("Error converting to canonical path"),
        ) // TODO: Return result.
    })
}

pub fn find_unsafe_in_string(
    src: &str,
    include_tests: IncludeTests,
) -> Result<RsFileMetrics, syn::Error> {
    use syn::visit::Visit;
    let syntax = syn::parse_file(&src)?;
    let mut vis = GeigerSynVisitor::new(include_tests);
    vis.visit_file(&syntax);
    Ok(vis.metrics)
}

/// Scan a single file for `unsafe` usage.
pub fn find_unsafe_in_file(
    p: &Path,
    include_tests: IncludeTests,
) -> Result<RsFileMetrics, ScanFileError> {
    let mut file =
        File::open(p).map_err(|e| ScanFileError::Io(e, p.to_path_buf()))?;
    let mut src = vec![];
    file.read_to_end(&mut src)
        .map_err(|e| ScanFileError::Io(e, p.to_path_buf()))?;
    let src = String::from_utf8(src)
        .map_err(|e| ScanFileError::Utf8(e, p.to_path_buf()))?;
    find_unsafe_in_string(&src, include_tests)
        .map_err(|e| ScanFileError::Syn(e, p.to_path_buf()))
}
