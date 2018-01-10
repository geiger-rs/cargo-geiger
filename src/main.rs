extern crate syn;

use std::env;
use std::fs::File;
use std::io::Read;
use std::process;

fn foo() {
    unsafe {
        println!("Hello world!");
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
    println!("{:#?}", syntax);
}
