//! The outer CLI parts of the `cargo-geiger` cargo plugin executable.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![forbid(clippy::undocumented_unsafe_blocks)]

mod args;
use args::GeigerCli;
//use cargo_geiger::args::GeigerCli;
use cargo_geiger::Geiger;
use cargo_geiger::GeigerOpts;

#[allow(unused_imports)]
use log::{debug, error, log_enabled, info, Level};


fn main() {

    env_logger::init();
    
    //let geiger_args = cargo_geiger::args::Geiger::parse();

    let args = match GeigerCli::from_cli() {
        Ok(i) => i,
        Err(e) => e.exit() // clap::Error::exit()
    };

    debug!("Geiger args = {:?}", args);

    let geiger = Geiger::from_opts(args);
    
    let report = match geiger.run() {
        Ok(i) => i,
        Err(e) => panic!("Geiger Error: {:#?}", e),
    };

    
    debug!("Report = {:?}", report);
}
