extern crate syn;
extern crate clap;

use std::fmt;
use std::fs::File;
use std::io::Read;

use clap::{Arg, App};
use syn::{visit, ItemFn, Expr, ExprUnsafe, ItemImpl, ItemTrait, ImplItemMethod};

unsafe fn foo() {
    unsafe {
        let a = 10;
        println!("Bar");
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Count {
    num: u64,
    unsafe_num: u64,
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.unsafe_num, self.num)
    }
}


#[derive(Debug, Copy, Clone, Default)]
pub struct UnsafeCounter {
    functions: Count,

    exprs: Count,

    itemimpls: Count,

    itemtraits: Count,

    methods: Count,

    in_unsafe_block: bool,
}

impl<'ast> visit::Visit<'ast> for UnsafeCounter {
    fn visit_item_fn(&mut self, i: &ItemFn) {
        // fn definitions
        self.functions.count(i.unsafety.is_some());
        visit::visit_item_fn(self, i);
    }

    fn visit_expr(&mut self, i: &Expr) {
        // Total number of expressions of any type
        self.exprs.count(self.in_unsafe_block);
        visit::visit_expr(self, i);
    }

    fn visit_expr_unsafe(&mut self, i: &ExprUnsafe) {
        // unsafe {} expression blocks
        self.in_unsafe_block = true;
        visit::visit_expr_unsafe(self, i);
        self.in_unsafe_block = false;
    }

    fn visit_item_impl(&mut self, i: &ItemImpl) {
        // unsafe trait impl's
        self.itemimpls.count(i.unsafety.is_some());
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, i: &ItemTrait) {
        // Unsafe traits
        self.itemtraits.count(i.unsafety.is_some());
        visit::visit_item_trait(self, i);

    }

    fn visit_impl_item_method(&mut self, i: &ImplItemMethod) {
        self.methods.count(i.sig.unsafety.is_some());
        visit::visit_impl_item_method(self, i);
    }
}

impl fmt::Display for UnsafeCounter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Unsafe functions: {}", self.functions)?;
        writeln!(f, "Unsafe expressions: {}", self.exprs)?;
        writeln!(f, "Unsafe traits: {}", self.itemtraits)?;
        writeln!(f, "Unsafe methods: {}", self.methods)?;
        write!(f, "Unsafe impls: {}", self.itemimpls)
    }
}

fn main() {
    let matches = App::new("cargo-osha")
        .about("Prints statistics on the number of `unsafe` blocks in a Rust file.")
        .arg(Arg::with_name("files")
             .required(true)
             .takes_value(true)
             .multiple(true)
             .help("Files to process")
        )
        .get_matches();

    let tracker = &mut UnsafeCounter::default();
    
    if let Some(v) = matches.values_of("files") {
        for filename in v {
            println!("Processing file {}", filename);
            let mut file = File::open(filename).expect("Unable to open file");
            
            let mut src = String::new();
            file.read_to_string(&mut src).expect("Unable to read file");
            
            let syntax = syn::parse_file(&src).expect("Unable to parse file");
            visit::visit_file(tracker, &syntax);
        }
    }
    println!("{}", tracker);
}
