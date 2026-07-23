mod cli;
mod config;
mod core;
mod network;
mod ui;

use clap::Parser;
use cli::{Cli, Commands};
use owo_colors::OwoColorize;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let alpm_handle = core::alpm_init::init_alpm()?;
    
    // fetch local db
    let local_db = alpm_handle.localdb();
    // let pkg_count = local_db.pkgs().len();
    
    match &cli.command {
        Commands::Install { packages } => {
            println!("✓ installing packages: {:?}", packages);
        }
        Commands::Remove { packages } => {
            println!("✗ removing packages: {:?}", packages);
        }
        Commands::Update => {
            let spinner = ui::progress::spinner("syncing package databases from mirrors...");

            let output = std::process::Command::new("pacman")
                .arg("-Sy")
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    spinner.finish_with_message(format!("{} repositories synced successfully.", "✓".green()));
                }
                Ok(out) => {
                    let err_msg = String::from_utf8_lossy(&out.stderr);
                    spinner.finish_with_message(format!("{} sync failed: {}", "✗".red(), err_msg.trim()));
                }
                Err(e) => {
                    spinner.finish_with_message(format!("{} failed to execute sync engine: {}", "✗".red(), e));
                }
            }
        }
        Commands::Search { query } => {
            println!("{} searching for '{}'...\n", "✓".green(), query.cyan());

            let mut found = false;

            for db in alpm_handle.syncdbs() {
                if let Ok(results) = db.search([query.as_str()].into_iter()) {
                    for pkg in results {
                        found = true;
                        let is_installed = local_db.pkg(pkg.name()).is_ok();

                        ui::formatter::print_search_result(&pkg, db.name(), is_installed);
                    }
                }
            }

            if !found {
                println!("{} no packages found matching '{}'.", "✗".red(), query.bold());
            }
        }
        Commands::Show { package } => {
            // searches db for pkg
            match local_db.pkg(package.as_str()) {
                Ok(pkg) => {
                    println!("found locally: {} v{}", pkg.name(), pkg.version());
                    println!("description: {}", pkg.desc().unwrap_or("None"));
                }
                Err(_) => {
                    println!("✗ package '{}' not found in local database.", package);
                }
            }
        }
        Commands::Clean => {
            println!("✓ cleaning package cache...");
        }
        Commands::Orphan => {
            println!("✓ checking orphaned packages...");
        }
    }

    Ok(())
}
