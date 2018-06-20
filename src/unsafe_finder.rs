extern crate syn;
extern crate walkdir;

use std;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use self::syn::visit;
use self::walkdir::WalkDir;

/*
I'm guessing this is intended for testing?
Removing to avoid warnings for now.

unsafe fn foo() {
    unsafe {
        let a = 10;
        println!("Bar");
    }
}
*/

#[derive(Debug, Copy, Clone, Default)]
pub struct Count {
    pub num: u64,
    pub unsafe_num: u64,
}

impl Count {
    fn count(&mut self, is_unsafe: bool) {
        self.num += 1;
        if is_unsafe {
            self.unsafe_num += 1
        }
    }
}

impl fmt::Display for Count {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.unsafe_num, self.num)
    }
}


#[derive(Debug, Copy, Clone, Default)]
pub struct UnsafeCounter {
    pub functions: Count,
    pub exprs: Count,
    pub itemimpls: Count,
    pub itemtraits: Count,
    pub methods: Count,
    pub in_unsafe_block: bool,
}

impl<'ast> visit::Visit<'ast> for UnsafeCounter {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        // fn definitions
        self.functions.count(i.unsafety.is_some());
        visit::visit_item_fn(self, i);
    }

    fn visit_expr(&mut self, i: &'ast syn::Expr) {
        // Total number of expressions of any type
        self.exprs.count(self.in_unsafe_block);
        visit::visit_expr(self, i);
    }

    fn visit_expr_unsafe(&mut self, i: &'ast syn::ExprUnsafe) {
        // unsafe {} expression blocks
        self.in_unsafe_block = true;
        visit::visit_expr_unsafe(self, i);
        self.in_unsafe_block = false;
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        // unsafe trait impl's
        self.itemimpls.count(i.unsafety.is_some());
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        // Unsafe traits
        self.itemtraits.count(i.unsafety.is_some());
        visit::visit_item_trait(self, i);

    }

    fn visit_impl_item_method(&mut self, i: &'ast syn::ImplItemMethod) {
        self.methods.count(i.sig.unsafety.is_some());
        visit::visit_impl_item_method(self, i);
    }
}

impl fmt::Display for UnsafeCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        writeln!(f, "Unsafe functions: {}", self.functions)?;
        writeln!(f, "Unsafe expressions: {}", self.exprs)?;
        writeln!(f, "Unsafe traits: {}", self.itemtraits)?;
        writeln!(f, "Unsafe methods: {}", self.methods)?;
        write!(f, "Unsafe impls: {}", self.itemimpls)
    }
}

pub fn find_unsafe(p: &Path, allow_partial_results: bool) -> UnsafeCounter {
    /*
    let matches = App::new("cargo-osha")
        .about("Prints statistics on the number of `unsafe` blocks in a Rust file.")
        .arg(Arg::with_name("files")
             .required(true)
             .takes_value(true)
             .multiple(true)
             .help("Files to process")
        )
        .get_matches();
    */
    let tracker = &mut UnsafeCounter::default();
    let walker = WalkDir::new(p).into_iter();
    for entry in walker {
        let entry = entry.expect("walkdir error, TODO: Implement error handling");
        if !entry.file_type().is_file() {
            // TODO: Add --verbose flag and proper logging.
            // println!("Skipping non-file: {}", p.display());
            continue;
        }
        let p = entry.path();
        let ext = match p.extension() {
            Some(e) => e,
            None    => continue
        };
        // to_string_lossy is ok since we only want to match against an ASCII
        // compatible extension and we do not keep the possibly lossy result
        // around.
        if ext.to_string_lossy() != "rs" {
            // TODO: Add --verbose flag and proper logging.
            // println!("Skipping non-rust: {}", p.display());
            continue;
        }
        // TODO: Add --verbose flag and proper logging.
        // println!("Processing file {}", p.display());
        let mut file = File::open(p).expect("Unable to open file");
        let mut src = String::new();
        file.read_to_string(&mut src).expect("Unable to read file");
        let syntax = match (allow_partial_results, syn::parse_file(&src)) {
            (_, Ok(s)) => s,
            (true, Err(e)) => {
                // TODO: Do proper error logging.
                println!("Failed to parse file: {}, {:?}", p.display(), e);
                continue
            },
            (false, Err(e)) => panic!("Failed to parse file: {}, {:?} ", p.display(), e)
        };
        syn::visit::visit_file(tracker, &syntax);
    }
    *tracker
}
