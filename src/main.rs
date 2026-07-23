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
        println!("{} {}", "[dry run]".bold().yellow(), "no system changes will be made.");
        println!("{} would execute: pacman {}", "→".cyan(), args.join(" "));
        return;
    }

    let spinner = ui::progress::spinner(spinner_msg);
    let mut child = tokio::process::Command::new("pacman")
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn pacman");

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

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

    let status = loop {
        tokio::select! {
            Ok(Some(line)) = reader.next_line() => {
                let clean = line.trim();
                if clean.is_empty() { continue; }
                
                if clean.starts_with("::") {
                    spinner.set_message(format!("{}", clean.replace("::", "→").cyan().bold()));
                } else if clean.starts_with('(') || clean.contains("downloading") || clean.contains("installing") || clean.contains("removing") || clean.contains("upgrading") || clean.contains("cleaning") {
                    spinner.set_message(format!("  {}", clean.dimmed()));
                }
            }
            result = child.wait() => {
                break result; 
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
                "{} operation failed (code {}):\n{}", 
                "✗".red(), 
                stat.code().unwrap_or(1), 
                err_output.trim().red()
            ));
        }
        Err(e) => spinner.finish_with_message(format!("{} failed to execute pacman: {}", "✗".red(), e)),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let _config = config::load_config();

    match &cli.command {
        Commands::Update => {
            run_pacman(
                &["-Sy", "--noconfirm"], 
                "syncing package databases from mirrors...", 
                "repositories synced successfully.",
                cli.dry_run
            ).await;
        }
        
        cmd => {
            let alpm_handle = core::alpm_init::init_alpm()?;
            let local_db = alpm_handle.localdb();

            match cmd {
                Commands::Install { packages } => {
                    println!("{} resolving dependencies...\n", "✓".green());

                    match core::resolver::get_install_summaries(&alpm_handle, packages) {
                        Ok(summaries) => {
                            let mut total_dl = 0.0;
                            let mut total_inst = 0.0;

                            println!("{}", "installing".bold().white());
                            for sum in &summaries {
                                println!("  {} {}", sum.name.cyan().bold(), sum.version.dimmed());
                                total_dl += sum.download_size_mb;
                                total_inst += sum.install_size_mb;
                            }

                            println!("\n{:<15} {:.2} MB", "download:", total_dl);
                            println!("{:<15} {:.2} MB", "disk Usage:", total_inst);
                            
                            print!("\ncontinue? [y/n] ");
                            io::stdout().flush()?;

                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;

                            if input.trim().eq_ignore_ascii_case("y") || input.trim().is_empty() {
                                let mut args = vec!["-S", "--noconfirm"];
                                args.extend(packages.iter().map(|s| s.as_str()));
                                
                                // rop(local_db);
                                drop(alpm_handle);

                                run_pacman(&args, "initializing transaction...", "packages installed successfully.", cli.dry_run).await;
                            } else {
                                println!("{} aborted.", "✗".red());
                            }
                        }
                        Err(e) => println!("{} {}", "✗".red(), e),
                    }
                }
                
                Commands::Remove { packages } => {
                    println!("{} resolving targets...\n", "✓".green());
                    
                    let mut found = true;
                    for pkg in packages {
                        if local_db.pkg(pkg.as_str()).is_err() {
                            println!("{} package '{}' is not installed.", "✗".red(), pkg.bold());
                            found = false;
                        }
                    }

                    if !found {
                        println!("{} aborted.", "✗".red());
                        return Ok(());
                    }

                    println!("{}", "tossing".bold().white());
                    for pkg in packages {
                        println!("  {}", pkg.magenta().bold());
                    }

                    print!("\ncontinue? [y/n] ");
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    if input.trim().eq_ignore_ascii_case("y") {
                        let mut args = vec!["-Rs", "--noconfirm"];
                        args.extend(packages.iter().map(|s| s.as_str()));
                        
                        // drop(local_db);
                        drop(alpm_handle);

                        run_pacman(&args, "tossing packages back into the ocean...", "packages removed successfully.", cli.dry_run).await;
                    } else {
                        println!("{} aborted.", "✗".red());
                    }
                }
                
                Commands::Clean => {
                    // drop(local_db);
                    drop(alpm_handle);
                    
                    run_pacman(&["-Sc", "--noconfirm"], "scrubbing the package cache...", "cache cleared successfully.", cli.dry_run).await;
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
                    if !found { println!("{} no packages found matching '{}'.", "✗".red(), query.bold()); }
                }
                
                Commands::Show { package } => {
                    match local_db.pkg(package.as_str()) {
                        Ok(pkg) => {
                            println!("found locally: {} v{}", pkg.name(), pkg.version());
                            println!("description: {}", pkg.desc().unwrap_or("none"));
                        }
                        Err(_) => println!("{} package '{}' not found in local database.", "✗".red(), package),
                    }
                }
                
                Commands::Orphan => {
                    println!("{} scanning local database for orphans...\n", "✓".green());
                    let mut orphans = Vec::new();
                    
                    for pkg in local_db.pkgs() {
                        if pkg.reason() == alpm::PackageReason::Depend 
                           && pkg.required_by().is_empty() 
                           && pkg.optional_for().is_empty() 
                        { orphans.push(pkg); }
                    }

                    if orphans.is_empty() {
                        println!("{} no orphaned packages found. system is clean!", "✓".green());
                    } else {
                        println!("{}", "orphans found:".bold().white());
                        let mut total_size = 0.0;
                        for pkg in &orphans {
                            println!("  {:<25} {}", pkg.name().magenta(), pkg.version().dimmed());
                            total_size += pkg.isize() as f64 / 1024.0 / 1024.0;
                        }
                        println!("\n{:<15} {:.2} MB", "wasted space:", total_size);
                        println!("\nrun {} to remove them.", "haj toss <packages>".cyan());
                    }
                }
                Commands::Update => unreachable!(),
            }
        }
    }
    Ok(())
}
