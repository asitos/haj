mod cli;
mod config;
mod core;
mod network;
mod ui;

use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Install { packages } => {
            println!("[+] installing packages: {:?}", packages);
        }
        Commands::Remove { packages } => {
            println!("[-] removing packages: {:?}", packages);
        }
        Commands::Update => {
            println!("[*] syncing repositories and checking for upgrades...");
        }
        Commands::Search { query } => {
            println!("[?] searching for: {}", query);
        }
        Commands::Show { package } => {
            println!("[i] displaying info for: {}", package);
        }
        Commands::Clean => {
            println!("[*] cleaning package cache...");
        }
        Commands::Orphan => {
            println!("[*] checking orphaned packages...");
        }
    }

    Ok(())
}
