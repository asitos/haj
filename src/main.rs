mod cli;
mod config;
mod core;
mod network;
mod tui;
mod ui;

use clap::Parser;
use cli::{Cli, Commands};
use owo_colors::OwoColorize;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

async fn run_pacman(args: &[&str], spinner_msg: &str, success_msg: &str, is_dry_run: bool) {
    if is_dry_run {
        println!(
            "{} no system changes will be made.",
            "[dry run]".bold().yellow()
        );
        let arrow = "→".cyan();
        let cmd = args.join(" ");
        println!("{arrow} would execute: pacman {cmd}");
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
        Err(e) => {
            spinner.finish_with_message(format!("{} failed to execute pacman: {}", "✗".red(), e))
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let _config = config::load_config();

    match &cli.command {
        Commands::Tui => {
            tui::run().await?;
        }
        Commands::Update => {
            run_pacman(
                &["-Sy", "--noconfirm"],
                "syncing package databases from mirrors...",
                "repositories synced successfully.",
                cli.dry_run,
            )
            .await;
        }
        cmd => {
            let alpm_handle = core::alpm_init::init_alpm()?;
            let local_db = alpm_handle.localdb();

            match cmd {
                Commands::Tui => unreachable!(),
                Commands::Update => unreachable!(),
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
                            println!("{:<15} {:.2} MB", "disk usage:", total_inst);

                            print!("\ncontinue? [y/n] ");
                            io::stdout().flush()?;

                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;

                            if input.trim().eq_ignore_ascii_case("y") || input.trim().is_empty() {
                                let mut args = vec!["-S", "--noconfirm"];
                                args.extend(packages.iter().map(|s| s.as_str()));

                                drop(alpm_handle);

                                run_pacman(
                                    &args,
                                    "initializing transaction...",
                                    "packages installed successfully.",
                                    cli.dry_run,
                                )
                                .await;
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

                        drop(alpm_handle);

                        run_pacman(
                            &args,
                            "tossing packages back into the ocean...",
                            "packages removed successfully.",
                            cli.dry_run,
                        )
                        .await;
                    } else {
                        println!("{} aborted.", "✗".red());
                    }
                }

                Commands::Clean => {
                    drop(alpm_handle);

                    run_pacman(
                        &["-Sc", "--noconfirm"],
                        "scrubbing the package cache...",
                        "cache cleared successfully.",
                        cli.dry_run,
                    )
                    .await;
                }

                Commands::Search { query } => {
                    let search_repo = cli.repo || !cli.aur;
                    let search_aur = cli.aur || !cli.repo;

                    let target_msg = if cli.aur && !cli.repo {
                        "aur"
                    } else if cli.repo && !cli.aur {
                        "standard repos"
                    } else {
                        "standard repos and aur"
                    };

                    println!(
                        "{} searching {} for '{}'...\n",
                        "✓".green(),
                        target_msg.white().bold(),
                        query.cyan()
                    );
                    let mut found = false;

                    if search_repo {
                        for db in alpm_handle.syncdbs() {
                            if let Ok(results) = db.search([query.as_str()].into_iter()) {
                                for pkg in results {
                                    found = true;
                                    let is_installed = local_db.pkg(pkg.name()).is_ok();
                                    ui::formatter::print_search_result(
                                        pkg,
                                        db.name(),
                                        is_installed,
                                    );
                                }
                            }
                        }
                    }

                    if search_aur {
                        let aur_url = format!("https://aur.archlinux.org/rpc/v5/search/{}", query);

                        if let Ok(response) = reqwest::get(&aur_url).await
                            && let Ok(json) = response.json::<serde_json::Value>().await
                            && let Some(results) = json.get("results").and_then(|r| r.as_array())
                            && !results.is_empty()
                        {
                            if found {
                                println!();
                            }
                            found = true;

                            for pkg in results {
                                let name = pkg
                                    .get("Name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("unknown");
                                let version =
                                    pkg.get("Version").and_then(|v| v.as_str()).unwrap_or("");
                                let desc = pkg
                                    .get("Description")
                                    .and_then(|d| d.as_str())
                                    .unwrap_or("no description provided.");
                                let votes =
                                    pkg.get("NumVotes").and_then(|v| v.as_u64()).unwrap_or(0);

                                let is_installed = local_db.pkg(name).is_ok();
                                let install_marker = if is_installed {
                                    format!(" {}", "[installed]".cyan().bold())
                                } else {
                                    "".to_string()
                                };

                                println!(
                                    "{}/{} {} {}{}",
                                    "aur".magenta().bold(),
                                    name.bold(),
                                    version.green(),
                                    format!("(+{})", votes).yellow(),
                                    install_marker
                                );
                                println!("    {}", desc.dimmed());
                            }
                        }
                    }
                    if !found {
                        println!(
                            "\n{} no packages found matching '{}' in {}.",
                            "✗".red(),
                            query.bold(),
                            target_msg
                        );
                    }
                }

                Commands::Show { package } => match local_db.pkg(package.as_str()) {
                    Ok(pkg) => {
                        println!("found locally: {} v{}", pkg.name(), pkg.version());
                        println!("description: {}", pkg.desc().unwrap_or("none"));
                    }
                    Err(_) => println!(
                        "{} package '{}' not found in local database.",
                        "✗".red(),
                        package
                    ),
                },

                Commands::Orphan => {
                    println!("{} scanning local database for orphans...\n", "✓".green());
                    let mut orphans = Vec::new();

                    for pkg in local_db.pkgs() {
                        if pkg.reason() == alpm::PackageReason::Depend
                            && pkg.required_by().is_empty()
                            && pkg.optional_for().is_empty()
                        {
                            orphans.push(pkg);
                        }
                    }

                    if orphans.is_empty() {
                        println!(
                            "{} no orphaned packages found. system is clean!",
                            "✓".green()
                        );
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

                Commands::Owns { file_path } => {
                    core::alpm_init::owns(&alpm_handle, file_path);
                }

                Commands::Files { package } => {
                    core::alpm_init::files(&alpm_handle, package);
                }

                Commands::Load { archive_path } => {
                    drop(alpm_handle);

                    run_pacman(
                        &["-U", archive_path, "--noconfirm"],
                        "loading package archive...",
                        "package loaded successfully.",
                        cli.dry_run,
                    )
                    .await;
                }

                Commands::Fetch { packages } => {
                    drop(alpm_handle);

                    let mut args = vec!["-Sw", "--noconfirm"];
                    args.extend(packages.iter().map(|s| s.as_str()));

                    run_pacman(
                        &args,
                        "fetching packages to cache...",
                        "packages downloaded successfully.",
                        cli.dry_run,
                    )
                    .await;
                }

                Commands::Mark {
                    package,
                    as_explicit,
                } => {
                    drop(alpm_handle);

                    let reason_flag = if *as_explicit {
                        "--asexplicit"
                    } else {
                        "--asdeps"
                    };
                    let state = if *as_explicit {
                        "explicit"
                    } else {
                        "dependency"
                    };

                    run_pacman(
                        &["-D", reason_flag, package],
                        "updating database records...",
                        &format!("marked {} as {}.", package, state),
                        cli.dry_run,
                    )
                    .await;
                }

                Commands::Diff => {
                    drop(alpm_handle);
                    core::pacnew::manage_pacnew_files();
                }

                Commands::List { explicit, deps } => {
                    let mut count = 0;

                    for pkg in local_db.pkgs() {
                        let is_explicit = pkg.reason() == alpm::PackageReason::Explicit;

                        if *explicit && !is_explicit {
                            continue;
                        }
                        if *deps && is_explicit {
                            continue;
                        }

                        println!("{} {}", pkg.name().cyan(), pkg.version().dimmed());
                        count += 1;
                    }

                    println!("\n{} {} packages listed.", "✓".green(), count.bold());
                }

                Commands::Locate { query } => {
                    drop(alpm_handle);

                    println!(
                        "{} searching remote file databases for '{}'...\n",
                        "✓".green(),
                        query.cyan()
                    );

                    let status = std::process::Command::new("pacman")
                        .arg("-F")
                        .arg(query)
                        .status()
                        .expect("failed to execute pacman -F");

                    if !status.success() {
                        println!(
                            "\n{} no results found. (you may need to sync file databases with 'sudo pacman -Fy')",
                            "✗".red()
                        );
                    }
                }
            }
        }
    }
    Ok(())
}
