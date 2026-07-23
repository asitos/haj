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

    #[arg(short = 'd', long, global = true)]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(alias = "i")]
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    #[command(alias = "rm", alias = "toss")]
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    #[command(alias = "up", alias = "sync")]
    Update,

    #[command(alias = "s")]
    Search {
        query: String,
    },

    #[command(alias = "info")]
    Show {
        package: String,
    },

    #[command(alias = "c")]
    Clean,

    #[command(alias = "o")]
    Orphan,
}
