extern crate syn;

use std::env;
use std::fs::File;
use std::io::Read;
use std::process;

use syn::visit;

unsafe fn foo() {
    unsafe {
        println!("Bar");
    }
}

#[derive(Debug, Clone, Default)]
pub struct UnsafeTracker {
    functions: u64,
    unsafe_functions: u64,

    exprs: u64,
    unsafe_exprs: u64,

    itemimpls: u64,
    unsafe_itemimpls: u64,
}

impl<'ast> visit::Visit<'ast> for UnsafeTracker {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.functions += 1;
        if i.unsafety.is_some() {
            self.unsafe_functions += 1;
        }
    }

    fn visit_expr(&mut self, i: &'ast syn::Expr) {
        // Total number of expressions of any type
        self.exprs += 1;
        visit::visit_expr(self, i);
    }

    fn visit_expr_unsafe(&mut self, i: &'ast syn::ExprUnsafe) {
        // unsafe {} blocks
        // BUGGO: Doesn't seem to work?
        // BECAUSE, we have to actually recurse explicitly
        // to walk the syntax tree.  Bah.  Bah!
        self.unsafe_exprs += 1;
        visit::visit_expr_unsafe(self, i);
    }
}

fn main() {
    let mut args = env::args();
    let _ = args.next(); // executable name

    let filename = match (args.next(), args.next()) {
        (Some(filename), None) => filename,
        _ => {
            eprintln!("Usage: cargo run -- path/to/filename.rs");
            process::exit(1);
        }
    };

    let mut file = File::open(&filename).expect("Unable to open file");

    let mut src = String::new();
    file.read_to_string(&mut src).expect("Unable to read file");

    let syntax = syn::parse_file(&src).expect("Unable to parse file");
    let tracker = &mut UnsafeTracker::default();
    syn::visit::visit_file(tracker, &syntax);
    println!("{:#?}", tracker);
}
