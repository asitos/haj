mod cli;
mod config;
mod core;
mod network;
mod ui;

use std::io::{self, Write};
use clap::Parser;
use cli::{Cli, Commands};
use owo_colors::OwoColorize;
use tokio::io::{AsyncBufReadExt, BufReader};

async fn run_pacman(args: &[&str], spinner_msg: &str, success_msg: &str, is_dry_run: bool) {
    if is_dry_run {
        println!("{} {}", "[DRY RUN]".bold().yellow(), "No system changes will be made.");
        println!("{} Would execute: pacman {}", "→".cyan(), args.join(" "));
        return;
    }

    let spinner = ui::progress::spinner(spinner_msg);
    let mut child = tokio::process::Command::new("pacman")
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn pacman");

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Background thread to capture raw errors
    let err_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut err_str = String::new();
        while let Ok(Some(line)) = reader.next_line().await {
            err_str.push_str(&line);
            err_str.push('\n');
        }
        err_str
    });

    let mut reader = BufReader::new(stdout).lines();

    // tokio::select! races the process against the output stream.
    // This makes it 100% immune to background worker pipe deadlocks.
    let status = loop {
        tokio::select! {
            Ok(Some(line)) = reader.next_line() => {
                let clean = line.trim();
                if clean.is_empty() { continue; }
                
                // Parse pacman's standard phrasing into Blaharch aesthetics
                if clean.starts_with("::") {
                    spinner.set_message(format!("{}", clean.replace("::", "→").cyan().bold()));
                } else if clean.starts_with('(') || clean.contains("downloading") || clean.contains("installing") || clean.contains("removing") || clean.contains("upgrading") || clean.contains("cleaning") {
                    spinner.set_message(format!("  {}", clean.dimmed()));
                }
            }
            result = child.wait() => {
                break result; // The exact millisecond pacman finishes, we break out.
            }
        }
    };

    let err_output = err_handle.await.unwrap_or_default();

    match status {
        Ok(stat) if stat.success() => {
            spinner.finish_with_message(format!("{} {}", "✓".green(), success_msg));
        }
        Ok(stat) => {
            spinner.finish_with_message(format!(
                "{} Operation failed (code {}):\n{}", 
                "✗".red(), 
                stat.code().unwrap_or(1), 
                err_output.trim().red()
            ));
        }
        Err(e) => spinner.finish_with_message(format!("{} Failed to execute pacman: {}", "✗".red(), e)),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // 1. ISOLATE THE UPDATE COMMAND
    // We do not load local_db here to prevent file lock collisions.
    match &cli.command {
        Commands::Update => {
            run_pacman(
                &["-Sy", "--noconfirm"], 
                "Syncing package databases from mirrors...", 
                "Repositories synced successfully.",
                cli.dry_run
            ).await;
        }
        
        // 2. HANDLE ALL OTHER COMMANDS
        cmd => {
            let alpm_handle = core::alpm_init::init_alpm()?;
            let local_db = alpm_handle.localdb();

            match cmd {
                Commands::Install { packages } => {
                    println!("{} Resolving dependencies...\n", "✓".green());

                    match core::resolver::get_install_summaries(&alpm_handle, packages) {
                        Ok(summaries) => {
                            let mut total_dl = 0.0;
                            let mut total_inst = 0.0;

                            println!("{}", "Installing".bold().white());
                            for sum in &summaries {
                                println!("  {} {}", sum.name.cyan().bold(), sum.version.dimmed());
                                total_dl += sum.download_size_mb;
                                total_inst += sum.install_size_mb;
                            }

                            println!("\n{:<15} {:.2} MB", "Download:", total_dl);
                            println!("{:<15} {:.2} MB", "Disk Usage:", total_inst);
                            
                            print!("\nContinue? [y/N] ");
                            io::stdout().flush()?;

                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;

                            if input.trim().eq_ignore_ascii_case("y") || input.trim().is_empty() {
                                let mut args = vec!["-S", "--noconfirm"];
                                args.extend(packages.iter().map(|s| s.as_str()));
                                
                                // SAFELY drop our lock on the database before handing control to pacman
                                drop(local_db);
                                drop(alpm_handle);

                                run_pacman(&args, "Initializing transaction...", "Packages installed successfully.", cli.dry_run).await;
                            } else {
                                println!("{} Aborted.", "✗".red());
                            }
                        }
                        Err(e) => println!("{} {}", "✗".red(), e),
                    }
                }
                
                Commands::Remove { packages } => {
                    println!("{} Resolving targets...\n", "✓".green());
                    
                    let mut found = true;
                    for pkg in packages {
                        if local_db.pkg(pkg.as_str()).is_err() {
                            println!("{} Package '{}' is not installed.", "✗".red(), pkg.bold());
                            found = false;
                        }
                    }

                    if !found {
                        println!("{} Aborted.", "✗".red());
                        return Ok(());
                    }

                    println!("{}", "Tossing".bold().white());
                    for pkg in packages {
                        println!("  {}", pkg.magenta().bold());
                    }

                    print!("\nContinue? [y/N] ");
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    if input.trim().eq_ignore_ascii_case("y") {
                        let mut args = vec!["-Rs", "--noconfirm"];
                        args.extend(packages.iter().map(|s| s.as_str()));
                        
                        // Drop locks so pacman can modify the DB
                        drop(local_db);
                        drop(alpm_handle);

                        run_pacman(&args, "Tossing packages back into the ocean...", "Packages removed successfully.", cli.dry_run).await;
                    } else {
                        println!("{} Aborted.", "✗".red());
                    }
                }
                
                Commands::Clean => {
                    // Drop locks so pacman can modify the cache
                    drop(local_db);
                    drop(alpm_handle);
                    
                    run_pacman(&["-Sc", "--noconfirm"], "Scrubbing the package cache...", "Cache cleared successfully.", cli.dry_run).await;
                }
                
                Commands::Search { query } => {
                    println!("{} Searching for '{}'...\n", "✓".green(), query.cyan());
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
                    if !found { println!("{} No packages found matching '{}'.", "✗".red(), query.bold()); }
                }
                
                Commands::Show { package } => {
                    match local_db.pkg(package.as_str()) {
                        Ok(pkg) => {
                            println!("Found locally: {} v{}", pkg.name(), pkg.version());
                            println!("Description: {}", pkg.desc().unwrap_or("None"));
                        }
                        Err(_) => println!("{} Package '{}' not found in local database.", "✗".red(), package),
                    }
                }
                
                Commands::Orphan => {
                    println!("{} Scanning local database for orphans...\n", "✓".green());
                    let mut orphans = Vec::new();
                    
                    for pkg in local_db.pkgs() {
                        if pkg.reason() == alpm::PackageReason::Depend 
                           && pkg.required_by().is_empty() 
                           && pkg.optional_for().is_empty() 
                        { orphans.push(pkg); }
                    }

                    if orphans.is_empty() {
                        println!("{} No orphaned packages found. System is clean!", "✓".green());
                    } else {
                        println!("{}", "Orphans found:".bold().white());
                        let mut total_size = 0.0;
                        for pkg in &orphans {
                            println!("  {:<25} {}", pkg.name().magenta(), pkg.version().dimmed());
                            total_size += pkg.isize() as f64 / 1024.0 / 1024.0;
                        }
                        println!("\n{:<15} {:.2} MB", "Wasted Space:", total_size);
                        println!("\nRun {} to remove them.", "haj toss <packages>".cyan());
                    }
                }
                Commands::Update => unreachable!(),
            }
        }
    }
    Ok(())
}
