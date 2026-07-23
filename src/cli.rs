use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "haj",
    author = "asitos",
    version = "0.1.0",
    about = "fast, quiet, beautiful package management for BlahArch.",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// preview a command without modifying the system
    #[arg(short = 'd', long, global = true)]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
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
