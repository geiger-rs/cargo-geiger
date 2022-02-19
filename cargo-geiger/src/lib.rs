//! These modules expose the internal workings of `cargo-geiger`. They
//! are currently not stable, and therefore have no associated `SemVer`.
//! As such, any function contained within may be subject to change.

#![deny(clippy::cargo)]
#![deny(clippy::doc_markdown)]
#![forbid(unsafe_code)]
#![deny(warnings)]

/// Functions for handling runners
pub mod runners;

//pub mod traits;

//use cargo_geiger::traits::GeigerOpts;

pub trait GeigerOpts {
    fn verbose(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum GeigerError {}

pub struct Geiger<R> {
    pub opts: R,
}

impl<R: GeigerOpts + std::fmt::Debug> Geiger<R> {
    pub fn from_opts(opts: R) -> Self {
        Self { opts: opts }
    }
    pub fn run(&self) -> Result<(), GeigerError> {
        println!("wee running. opts = {:?}", self.opts);
        println!("Verbosity: {:?}", self.opts.verbose());

        Ok(())
    }
}
