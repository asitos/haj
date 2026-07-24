use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "haj",
    author = "asitos",
    version = "0.2.0",
    about = "fast, quiet, beautiful package management for BlahArch.",
    long_about = None,
    disable_help_subcommand = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// enable verbose debug logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// preview a command without modifying the system
    #[arg(short = 'd', long, global = true)]
    pub dry_run: bool,

    /// display this help message
    #[arg(short = 'h', long, action = clap::ArgAction::Help)]
    pub help: Option<bool>,

    /// show version info
    #[arg(short = 'V', long, action = clap::ArgAction::Version)]
    pub version: Option<bool>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// launch the interactive tui dashboard
    #[command(alias = "t")]
    Tui,

    /// install one or more packages
    #[command(alias = "i")]
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    /// remove packages & unneeded dependencies
    #[command(alias = "rm", alias = "toss")]
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    /// sync mirror databases
    #[command(alias = "up", alias = "sync")]
    Update,

    /// search all remote sync databases
    #[command(alias = "s")]
    Search { query: String },

    /// show local package details
    #[command(alias = "info")]
    Show { package: String },

    /// scrub the package cache
    #[command(alias = "c")]
    Clean,

    /// detect orphaned dependencies
    #[command(alias = "o")]
    Orphan,
}
