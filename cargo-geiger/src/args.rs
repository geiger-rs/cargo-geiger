//! The CLI Arguments Parser using clap, clap-cargo

use clap::AppSettings;
use clap::Parser;
use clap::Subcommand;

//use std::path::PathBuf;
//use std::ffi::OsString;

#[derive(Parser, Debug)]
#[clap(name = "cargo geiger")]
#[clap(about, author, version, bin_name = "cargo geiger")]
#[clap(setting(AppSettings::DisableHelpSubcommand))]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
#[clap(global_setting(AppSettings::AllowMissingPositional))]
//#[clap(global_setting(AppSettings::SubcommandPrecedenceOverArg))]
//#[clap(global_setting(AppSettings::AllowExternalSubcommands))]
pub struct GeigerCli {
    #[clap(flatten)]
    pub geiger_args: GeigerArgs,
    #[clap(subcommand)]
    pub command: GeigerCommands,
}

impl GeigerCli {
    pub fn from_cli() -> Result<Self, clap::Error> {
        GeigerCli::try_parse()
    }
}

use crate::GeigerOpts;

impl GeigerOpts for GeigerCli {}

#[derive(Parser, Debug)]
#[clap(setting(AppSettings::DeriveDisplayOrder))]
pub struct GeigerArgs {
    #[clap(short, long, required = false, hide_long_help = false)]
    #[clap(default_value("0"))]
    /// Depth limit | 1 = Crate | >= 2 Dependencies
    depth: u32,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[clap(flatten)]
    workspace: clap_cargo::Workspace,
    #[clap(flatten)]
    manifest: clap_cargo::Manifest,
    #[clap(flatten)]
    features: clap_cargo::Features,
}

#[derive(Subcommand, Debug)]
#[clap(setting(AppSettings::DeriveDisplayOrder))]
pub enum GeigerCommands {
    /// Geiger with build tree
    Build {
        #[clap(
            last(true),
            global(true),
            allow_hyphen_values(true),
            required(false)
        )]
        /// Extra cargo arguments to proxy
        cargo_args: Vec<String>,
    },
    /// Geiger with test tree
    Test {},
    /// Geiger with runtime tree
    Run {},
}
