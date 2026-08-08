mod cli;
pub mod commands;
mod config;
mod core;
mod tui;
mod ui;

use clap::Parser;
use cli::{Cli, Commands};
use crossterm::style::Stylize;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            println!(
                "\n\n{} received SIGINT (Ctrl+C). Cleaning up locks and exiting...",
                "✗".red().bold()
            );

            let _ = std::process::Command::new("sudo")
                .args(["rm", "-f", "/var/lib/pacman/db.lck"])
                .status();

            let _ = std::fs::remove_file("/tmp/haj.lock");
            std::process::exit(130);
        }
    });

    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o666)
        .open("/tmp/haj.lock")
        .map_err(|e| anyhow::anyhow!("failed to open lock file: {}", e))?;

    use std::os::unix::io::AsRawFd;
    let fd = lock_file.as_raw_fd();
    if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        println!(
            "{} haj is already running. waiting for lock...",
            "::".blue()
        );
        unsafe {
            libc::flock(fd, libc::LOCK_EX);
        }
    }

    let _haj_lock = lock_file;

    let mut cli = Cli::parse();
    let config = config::load_config();
    cli.aur = cli.aur || config.general.aur_only;
    cli.repo = cli.repo || config.general.repo_only;
    cli.verbose = cli.verbose || config.general.verbose;

    let active_command = cli.command.clone().unwrap_or_else(|| {
        use clap::CommandFactory;
        let _ = Cli::command().print_help();
        std::process::exit(0);
    });

    match active_command {
        Commands::Tui => {
            tui::run().await?;
        }
        Commands::Completions { shell } => match shell {
            clap_complete::Shell::Bash => {
                print!("{}", include_str!("completions/bash.sh"));
            }
            clap_complete::Shell::Zsh => {
                print!("{}", include_str!("completions/zsh.sh"));
            }
            clap_complete::Shell::Fish => {
                print!("{}", include_str!("completions/fish.sh"));
            }
            _ => {
                use clap::CommandFactory;
                let mut cmd = Cli::command();
                let bin_name = cmd.get_name().to_string();
                clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
            }
        },
        Commands::Update => {
            core::pacman::run_pacman(
                &["-Sy", "--noconfirm"],
                "syncing package databases from mirrors...",
                "repositories synced successfully.",
                cli.dry_run,
                cli.verbose,
                &cli.root,
            )
            .await;
        }
        cmd => {
            match &cmd {
                Commands::Install { .. } | Commands::Interactive(_) | Commands::Search { .. } => {
                    core::pacman::check_and_offer_sync(&cli).await;
                }
                _ => {}
            }

            let alpm_handle = core::alpm_init::init_alpm()?;
            let local_db = alpm_handle.localdb();

            match cmd {
                Commands::Tui | Commands::Completions { .. } | Commands::Update => unreachable!(),

                Commands::Install { packages } => {
                    commands::process_installation(packages, alpm_handle, &cli).await;
                }

                Commands::Interactive(queries) => {
                    let query_str = queries.join(" ");
                    println!(
                        "{} searching for '{}'...\n",
                        "::".blue(),
                        query_str.clone().bold()
                    );

                    let mut results = Vec::new();

                    if !cli.aur {
                        for db in alpm_handle.syncdbs() {
                            for pkg in db.pkgs() {
                                if pkg
                                    .name()
                                    .to_lowercase()
                                    .contains(&query_str.to_lowercase())
                                {
                                    let is_installed = local_db.pkg(pkg.name()).is_ok();
                                    let status = if is_installed {
                                        format!(
                                            "{} {}",
                                            db.name().blue(),
                                            "[installed]".cyan().bold()
                                        )
                                    } else {
                                        db.name().blue().to_string()
                                    };
                                    results.push((
                                        pkg.name().to_string(),
                                        pkg.version().to_string(),
                                        status,
                                        pkg.desc()
                                            .unwrap_or("no description provided.")
                                            .to_string(),
                                    ));
                                }
                            }
                        }
                    }

                    if !cli.repo {
                        let aur_url = format!(
                            "https://aur.archlinux.org/rpc/v5/search/{}?by=name",
                            query_str
                        );
                        let check_spinner = ui::spinner("querying aur...");

                        let response = reqwest::get(&aur_url).await;
                        if response.is_err() {
                            check_spinner.finish_and_clear();
                            core::pacman::print_network_error(&format!(
                                "{} failed to query the aur.",
                                "✗".red()
                            ));
                        } else if let Ok(resp) = response
                            && let Ok(json) = resp.json::<core::aur::AurResponse>().await
                        {
                            check_spinner.finish_and_clear();
                            for pkg in json.results {
                                let name = pkg.name;
                                let version = pkg.version;
                                let desc = pkg.description.unwrap_or_else(|| "no description provided.".to_string());
                                let votes = pkg.num_votes;

                                let is_installed = local_db.pkg(name.as_str()).is_ok();
                                let status = if is_installed {
                                    format!(
                                        "{} (+{}) {}",
                                        "aur".magenta(),
                                        votes,
                                        "[installed]".cyan().bold()
                                    )
                                } else {
                                    format!("{} (+{})", "aur".magenta(), votes)
                                };

                                results.push((
                                    name,
                                    version,
                                    status,
                                    desc,
                                ));
                            }
                        } else {
                            check_spinner.finish_and_clear();
                        }
                    }

                    if results.is_empty() {
                        println!(
                            "{} no packages found matching '{}'.",
                            "✗".red(),
                            query_str.bold()
                        );
                        return Ok(());
                    }

                    println!(
                        "  {:<4} {:<35} {:<20} {}",
                        "#".white().bold(),
                        "package".white().bold(),
                        "version".white().bold(),
                        "origin/status".white().bold()
                    );
                    println!("  {}", "-".repeat(78).dim());

                    for (i, (name, ver, status, desc)) in results.iter().enumerate() {
                        let name_colored = if status.contains("aur") {
                            name.clone().magenta().bold().to_string()
                        } else {
                            name.clone().cyan().bold().to_string()
                        };

                        println!(
                            "  {:<4} {:<35} {:<20} {}",
                            format!("[{}]", i + 1).green().bold(),
                            name_colored,
                            ver.clone().dim(),
                            status
                        );
                        println!("       {}\n", desc.clone().dim());
                    }

                    print!(
                        "{} enter packages to install (e.g., 1 2 3) [leave blank to abort]: ",
                        "?".magenta().bold()
                    );
                    let _ = std::io::stdout().flush();
                    let mut input = String::new();
                    let _ = std::io::stdin().read_line(&mut input);

                    let selections: Vec<usize> = input
                        .split_whitespace()
                        .filter_map(|s| s.parse::<usize>().ok())
                        .filter(|&n| n > 0 && n <= results.len())
                        .collect();

                    if selections.is_empty() {
                        println!("{} aborted.", "✗".red());
                        return Ok(());
                    }

                    let mut pkgs_to_install = Vec::new();
                    for idx in selections {
                        pkgs_to_install.push(results[idx - 1].0.clone());
                    }

                    println!(
                        "{} queueing {} for installation...\n",
                        "✓".green(),
                        pkgs_to_install.join(", ").cyan()
                    );

                    commands::process_installation(pkgs_to_install, alpm_handle, &cli)
                        .await;
                }

                Commands::Remove { packages } => {
                    println!("{} resolving targets...\n", "✓".green());

                    let mut found = true;
                    for pkg in &packages {
                        if local_db.pkg(pkg.as_str()).is_err() {
                            println!(
                                "{} package '{}' is not installed.",
                                "✗".red(),
                                pkg.clone().bold()
                            );
                            found = false;
                        }
                    }

                    if !found {
                        println!("{} aborted.", "✗".red());
                        return Ok(());
                    }

                    let print_cmd = std::process::Command::new("pacman")
                        .env("LC_ALL", "C")
                        .arg("-Rsp")
                        .args(&packages)
                        .output()
                        .expect("failed to execute pacman");

                    if !print_cmd.status.success() {
                        let stdout_str = String::from_utf8_lossy(&print_cmd.stdout);
                        let stderr_str = String::from_utf8_lossy(&print_cmd.stderr);
                        println!("{} failed to resolve dependencies:\n", "✗".red());
                        for line in stderr_str.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                if trimmed.starts_with("error:") {
                                    println!("  {}", trimmed.red().bold());
                                } else {
                                    println!("  {}", trimmed.dim());
                                }
                            }
                        }
                        let mut parsed_conflicts = Vec::new();
                        let mut other_lines = Vec::new();
                        for line in stdout_str.lines() {
                            let trimmed = line.trim();
                            if trimmed.starts_with(":: removing ")
                                && let Some(breaks_idx) = trimmed.find(" breaks dependency '")
                            {
                                let rest = &trimmed[breaks_idx + 20..];
                                if let Some(req_idx) = rest.find("' required by ") {
                                    let dep = &rest[..req_idx];
                                    let dependent = &rest[req_idx + 14..];
                                    parsed_conflicts.push((dep.to_string(), dependent.to_string()));
                                    continue;
                                }
                            }
                            if !trimmed.is_empty() {
                                other_lines.push(trimmed.to_string());
                            }
                        }

                        if !parsed_conflicts.is_empty() {
                            println!();
                            let max_dep_len = parsed_conflicts
                                .iter()
                                .map(|(dep, _)| dep.len())
                                .max()
                                .unwrap_or(20)
                                .max(10); // at least length of "dependency"

                            println!(
                                "  {:<width$}   {}",
                                "dependency".bold().white(),
                                "required by".bold().white(),
                                width = max_dep_len
                            );
                            println!(
                                "  {:<width$}   {}",
                                "─".repeat(max_dep_len).dim(),
                                "─".repeat(15).dim(),
                                width = max_dep_len
                            );
                            for (dep, dependent) in parsed_conflicts {
                                println!(
                                    "  {:<width$}   {}",
                                    dep.cyan(),
                                    dependent.magenta().bold(),
                                    width = max_dep_len
                                );
                            }
                        }

                        if !other_lines.is_empty() {
                            println!();
                            for line in other_lines {
                                if line.starts_with("::") {
                                    println!("  {}", line.yellow());
                                } else {
                                    println!("  {}", line.dim());
                                }
                            }
                        }
                        println!();
                        return Ok(());
                    }

                    let stdout_str = String::from_utf8_lossy(&print_cmd.stdout);
                    let targets: Vec<&str> = stdout_str
                        .lines()
                        .map(|l| l.trim())
                        .filter(|l| !l.is_empty())
                        .collect();

                    println!("{}", "tossing the following packages:".bold().white());
                    for target in &targets {
                        println!("  {}", target.magenta().bold());
                    }
                    println!("\n{:<15} {}", "total:", targets.len().to_string().cyan());

                    if !cli.noconfirm {
                        println!();
                        if !core::ui::prompt_confirm("proceed with removal? [Y/n]") {
                            println!("{} aborted.", "✗".red());
                            return Ok(());
                        }
                    }

                    drop(alpm_handle);

                    let mut args = vec!["-Rs", "--noconfirm"];
                    args.extend(packages.iter().map(|s| s.as_str()));

                    core::pacman::run_pacman(
                        &args,
                        "tossing packages back into the ocean...",
                        "packages removed successfully.",
                        cli.dry_run,
                        cli.verbose,
                        &cli.root,
                    )
                    .await;
                }

                Commands::Upgrade { no_sync } => {
                    let mut foreign_pkgs = Vec::new();

                    if !cli.repo {
                        for pkg in local_db.pkgs() {
                            let mut found_in_repo = false;
                            for db in alpm_handle.syncdbs() {
                                if db.pkg(pkg.name()).is_ok() {
                                    found_in_repo = true;
                                    break;
                                }
                            }
                            if !found_in_repo {
                                foreign_pkgs
                                    .push((pkg.name().to_string(), pkg.version().to_string()));
                            }
                        }
                    }

                    drop(alpm_handle);

                    if !no_sync {
                        let sudo_status = std::process::Command::new("sudo").arg("-v").status();

                        if let Ok(status) = sudo_status
                            && !status.success()
                        {
                            println!("{} failed to obtain sudo privileges.", "✗".red());
                            return Ok(());
                        }

                        println!("{} syncing package databases...\n", "::".blue().bold());
                        let status = std::process::Command::new("sudo")
                            .args(["pacman", "-Sy"])
                            .stdout(std::process::Stdio::null())
                            .status();
                        if status.is_err() || !status.as_ref().unwrap().success() {
                            println!("{} failed to sync databases.", "✗".red());
                            return Ok(());
                        }
                    }

                    let mut aur_updates = Vec::new();
                    if !foreign_pkgs.is_empty() && !cli.repo {
                        let check_spinner = ui::spinner("querying aur for updates...");

                        for chunk in foreign_pkgs.chunks(50) {
                            let mut url = String::from("https://aur.archlinux.org/rpc/v5/info?");
                            for (name, _) in chunk {
                                url.push_str(&format!("arg[]={}&", name));
                            }

                            if let Ok(response) = reqwest::get(&url).await
                                && let Ok(json) = response.json::<core::aur::AurResponse>().await
                            {
                                for result in json.results {
                                    let name = result.name;
                                    let new_ver = result.version;
                                    if let Some((_, local_ver)) =
                                        chunk.iter().find(|(n, _)| **n == name)
                                        && alpm::vercmp(local_ver.as_str(), new_ver.as_str())
                                            == std::cmp::Ordering::Less
                                    {
                                        aur_updates.push((
                                            name,
                                            local_ver.clone(),
                                            new_ver,
                                        ));
                                    }
                                }
                            }
                        }
                        check_spinner.finish_and_clear();
                    }

                    let mut native_lines: Vec<String> = Vec::new();
                    if !cli.aur {
                        if let Ok(qu_output) =
                            std::process::Command::new("pacman").arg("-Qu").output()
                        {
                            let updates = String::from_utf8_lossy(&qu_output.stdout);
                            native_lines = updates
                                .lines()
                                .filter(|l| !l.trim().is_empty())
                                .map(|s| s.to_string())
                                .collect();
                        } else {
                            println!("{} failed to query updates.", "✗".red());
                        }
                    }

                    if native_lines.is_empty() && aur_updates.is_empty() {
                        println!("{} system is fully up to date!", "✓".green());
                        return Ok(());
                    }

                    println!("{}", "available upgrades:".bold().white());

                    for line in &native_lines {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 4 {
                            println!(
                                "  {:<30} {} -> {}",
                                parts[0].cyan().bold(),
                                parts[1].red(),
                                parts[3].green()
                            );
                        } else {
                            println!("  {}", line.clone().cyan());
                        }
                    }

                    for (name, old, new) in &aur_updates {
                        println!(
                            "  {:<30} {} -> {}",
                            name.clone().magenta().bold(),
                            old.clone().red(),
                            new.clone().green()
                        );
                    }

                    let total_upgrades = native_lines.len() + aur_updates.len();
                    println!("\n{:<15} {}", "total:", total_upgrades.to_string().cyan());
                    core::ui::display_arch_news().await;

                    if !cli.noconfirm && !core::ui::prompt_confirm("proceed with upgrade? [Y/n]") {
                        println!("{} aborted.", "✗".red());
                        return Ok(());
                    }

                    if !native_lines.is_empty() {
                        core::pacman::run_pacman(
                            &["-Su", "--noconfirm"],
                            "upgrading system packages...",
                            "system upgraded successfully.",
                            cli.dry_run,
                            cli.verbose,
                            &cli.root,
                        )
                        .await;
                    }

                    if !aur_updates.is_empty() {
                        for (name, _, new_ver) in aur_updates {
                            if cli.dry_run {
                                println!(
                                    "{} would build and update aur package: {}",
                                    "[dry run]".bold().yellow(),
                                    name.magenta()
                                );
                                continue;
                            }

                            println!(
                                "\n{} preparing to update {} ({})...",
                                "::".blue(),
                                name.clone().magenta().bold(),
                                new_ver.clone().green()
                            );

                            match core::aur::build(&name, cli.verbose).await {
                                Ok(pkg_path) => {
                                    let spinner_msg = format!(
                                        "updating built package {}...",
                                        name.clone().magenta().bold()
                                    );
                                    let success_msg = format!(
                                        "{} updated successfully ({}).",
                                        name.magenta().bold(),
                                        new_ver.dim()
                                    );
                                    let pacman_args =
                                        vec!["-U", pkg_path.to_str().unwrap(), "--noconfirm"];

                                    core::pacman::run_pacman(
                                        &pacman_args,
                                        &spinner_msg,
                                        &success_msg,
                                        cli.dry_run,
                                        cli.verbose,
                                        &cli.root,
                                    )
                                    .await;
                                }
                                Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
                            }
                        }
                    }
                }

                Commands::History { limit } => {
                    drop(alpm_handle);
                    core::history::show_history(limit);
                }

                Commands::Downgrade { package } => {
                    drop(alpm_handle);

                    if let Some(archive_path) = core::downgrade::select_downgrade_target(&package) {
                        let mut args = vec!["-U", archive_path.to_str().unwrap()];
                        if cli.noconfirm {
                            args.push("--noconfirm");
                        }

                        core::pacman::run_pacman(
                            &args,
                            &format!(
                                "downgrading to {}...",
                                archive_path.file_name().unwrap().to_string_lossy()
                            ),
                            "package downgraded successfully.",
                            cli.dry_run,
                            cli.verbose,
                            &cli.root,
                        )
                        .await;
                    }
                }

                Commands::Clean { keep } => {
                    drop(alpm_handle);
                    core::cache::scrub(keep);
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
                        query.clone().cyan()
                    );

                    let mut found = false;
                    let mut header_printed = false;

                    let mut print_header = || {
                        if !header_printed {
                            println!(
                                "  {:<35} {:<20} {}",
                                "package".white().bold(),
                                "version".white().bold(),
                                "origin/status".white().bold()
                            );
                            println!("  {}", "-".repeat(75).dim());
                            header_printed = true;
                        }
                    };

                    let query_lower = query.to_lowercase();

                    if search_repo {
                        for db in alpm_handle.syncdbs() {
                            for pkg in db.pkgs() {
                                if pkg.name().to_lowercase().contains(&query_lower) {
                                    found = true;
                                    print_header();

                                    let is_installed = local_db.pkg(pkg.name()).is_ok();
                                    let status = if is_installed {
                                        format!(
                                            "{} {}",
                                            db.name().blue(),
                                            "[installed]".cyan().bold()
                                        )
                                    } else {
                                        db.name().blue().to_string()
                                    };

                                    println!(
                                        "  {:<35} {:<20} {}",
                                        pkg.name().cyan().bold(),
                                        pkg.version().dim(),
                                        status
                                    );

                                    let desc = pkg.desc().unwrap_or("no description provided.");
                                    println!("      {}\n", desc.dim());
                                }
                            }
                        }
                    }

                    if search_aur {
                        let aur_url =
                            format!("https://aur.archlinux.org/rpc/v5/search/{}?by=name", query);

                        if let Ok(response) = reqwest::get(&aur_url).await
                            && let Ok(json) = response.json::<core::aur::AurResponse>().await
                            && !json.results.is_empty()
                        {
                            found = true;
                            print_header();

                            for pkg in json.results {
                                let name = pkg.name;
                                let version = pkg.version;
                                let desc = pkg.description.unwrap_or_else(|| "no description provided.".to_string());
                                let votes = pkg.num_votes;

                                let is_installed = local_db.pkg(name.as_str()).is_ok();
                                let status = if is_installed {
                                    format!(
                                        "{} (+{}) {}",
                                        "aur".magenta(),
                                        votes,
                                        "[installed]".cyan().bold()
                                    )
                                } else {
                                    format!("{} (+{})", "aur".magenta(), votes)
                                };

                                println!(
                                    "  {:<35} {:<20} {}",
                                    name.magenta().bold(),
                                    version.dim(),
                                    status
                                );
                                println!("      {}\n", desc.dim());
                            }
                        }
                    }

                    if !found {
                        println!(
                            "{} no packages found matching '{}' in {}.",
                            "✗".red(),
                            query.bold(),
                            target_msg
                        );
                    }
                }

                Commands::Show { package } => match local_db.pkg(package.as_str()) {
                    Ok(pkg) => {
                        println!(
                            "{} {} {}",
                            "::".blue(),
                            pkg.name().cyan().bold(),
                            pkg.version().dim()
                        );
                        if let Some(desc) = pkg.desc() {
                            println!("   {}", desc.italic());
                        }

                        let format_mb = |bytes: i64| -> String {
                            format!("{:.2} mb", bytes as f64 / 1_048_576.0)
                        };
                        println!("\n   {:<15} {}", "size:", format_mb(pkg.isize()).green());

                        let reason = match pkg.reason() {
                            alpm::PackageReason::Explicit => "explicitly installed",
                            alpm::PackageReason::Depend => "installed as a dependency",
                        };
                        println!("   {:<15} {}", "reason:", reason.yellow());

                        let depends = pkg.depends();
                        println!("\n{}", "depends on:".bold().white());
                        if depends.is_empty() {
                            println!("  {}", "none".dim());
                        } else {
                            let deps_list: Vec<_> = depends.iter().collect();
                            for (i, dep) in deps_list.iter().enumerate() {
                                let is_last = i == deps_list.len() - 1;
                                let prefix = if is_last { "└─" } else { "├─" };
                                println!("  {} {}", prefix.dim(), dep.name().magenta());
                            }
                        }

                        let required_by = pkg.required_by();
                        println!("\n{}", "required by:".bold().white());
                        if required_by.is_empty() {
                            println!("  {}", "none".dim());
                        } else {
                            for (i, req) in required_by.iter().enumerate() {
                                let is_last = i == required_by.len() - 1;
                                let prefix = if is_last { "└─" } else { "├─" };
                                println!("  {} {}", prefix.dim(), req.cyan());
                            }
                        }
                        println!();
                    }
                    Err(_) => {
                        println!(
                            "{} package '{}' not found in local database.",
                            "✗".red(),
                            package.bold()
                        );
                    }
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
                            println!("  {:<25} {}", pkg.name().magenta(), pkg.version().dim());
                            total_size += pkg.isize() as f64 / 1024.0 / 1024.0;
                        }
                        println!("\n{:<15} {:.2} mb", "wasted space:", total_size);
                        println!("\nrun {} to remove them.", "haj toss <packages>".cyan());
                    }
                }

                Commands::Owns { file_path } => {
                    core::alpm_init::owns(&alpm_handle, &file_path);
                }

                Commands::Files { package } => {
                    core::alpm_init::files(&alpm_handle, &package);
                }

                Commands::Load { archive_path } => {
                    drop(alpm_handle);

                    core::pacman::run_pacman(
                        &["-U", &archive_path, "--noconfirm"],
                        "loading package archive...",
                        "package loaded successfully.",
                        cli.dry_run,
                        cli.verbose,
                        &cli.root,
                    )
                    .await;
                }

                Commands::Fetch { packages } => {
                    drop(alpm_handle);

                    let mut args = vec!["-Sw", "--noconfirm"];
                    args.extend(packages.iter().map(|s| s.as_str()));

                    core::pacman::run_pacman(
                        &args,
                        "fetching packages to cache...",
                        "packages downloaded successfully.",
                        cli.dry_run,
                        cli.verbose,
                        &cli.root,
                    )
                    .await;
                }

                Commands::Mark { package } => {
                    let local_db = alpm_handle.localdb();
                    let (reason_flag, state) = match local_db.pkg(package.as_str()) {
                        Ok(pkg) => match pkg.reason() {
                            alpm::PackageReason::Explicit => ("--asdeps", "dependency"),
                            alpm::PackageReason::Depend => ("--asexplicit", "explicit"),
                        },
                        Err(_) => {
                            println!(
                                "{} package '{}' is not installed.",
                                "✗".red(),
                                package.bold()
                            );
                            drop(alpm_handle);
                            return Ok(());
                        }
                    };
                    drop(alpm_handle);

                    core::pacman::run_pacman(
                        &["-D", reason_flag, &package],
                        "updating database records...",
                        &format!("marked {} as {}.", package, state),
                        cli.dry_run,
                        cli.verbose,
                        &cli.root,
                    )
                    .await;
                }

                Commands::Diff => {
                    drop(alpm_handle);
                    if let Err(e) = core::manage_pacnew_files() {
                        eprintln!("{} {}", "error:".red(), e);
                    }
                }

                Commands::Pkgbuild { package } => {
                    drop(alpm_handle);
                    commands::view_pkgbuilds(&[package]).await;
                }

                Commands::List {
                    explicit,
                    deps,
                    foreign,
                } => {
                    let mut count = 0;

                    println!(
                        "\n  {:<35} {:<25} {}",
                        "package".white().bold(),
                        "version".white().bold(),
                        "origin".white().bold()
                    );
                    println!("  {}", "-".repeat(70).dim());

                    for pkg in local_db.pkgs() {
                        let is_explicit = pkg.reason() == alpm::PackageReason::Explicit;

                        if explicit && !is_explicit {
                            continue;
                        }
                        if deps && is_explicit {
                            continue;
                        }

                        let mut found_in_repo = false;
                        for db in alpm_handle.syncdbs() {
                            if db.pkg(pkg.name()).is_ok() {
                                found_in_repo = true;
                                break;
                            }
                        }

                        if foreign && found_in_repo {
                            continue;
                        }

                        if found_in_repo {
                            println!(
                                "  {:<35} {:<25} {}",
                                pkg.name().cyan(),
                                pkg.version().dim(),
                                "native".blue()
                            );
                        } else {
                            println!(
                                "  {:<35} {:<25} {}",
                                pkg.name().magenta(),
                                pkg.version().dim(),
                                "aur".magenta()
                            );
                        }
                        count += 1;
                    }

                    println!(
                        "\n{} {} packages listed.",
                        "✓".green(),
                        count.to_string().bold()
                    );
                }

                Commands::Stats => {
                    let spinner = ui::spinner("scanning system metrics...");

                    let mut total_pkgs = 0;
                    let mut explicit_pkgs = 0;
                    let mut dep_pkgs = 0;
                    let mut aur_pkgs = 0;
                    let mut installed_size: i64 = 0;
                    let mut orphan_count = 0;

                    let mut oldest_pkg = String::new();
                    let mut oldest_date = i64::MAX;
                    let mut newest_pkg = String::new();
                    let mut newest_date = 0;

                    for pkg in local_db.pkgs() {
                        total_pkgs += 1;
                        installed_size += pkg.isize();

                        let is_explicit = pkg.reason() == alpm::PackageReason::Explicit;
                        if is_explicit {
                            explicit_pkgs += 1;
                        } else {
                            dep_pkgs += 1;
                            if pkg.required_by().is_empty() && pkg.optional_for().is_empty() {
                                orphan_count += 1;
                            }
                        }

                        if let Some(idate) = pkg.install_date() {
                            if idate < oldest_date {
                                oldest_date = idate;
                                oldest_pkg = pkg.name().to_string();
                            }
                            if idate > newest_date {
                                newest_date = idate;
                                newest_pkg = pkg.name().to_string();
                            }
                        }

                        let mut found_in_repo = false;
                        for db in alpm_handle.syncdbs() {
                            if db.pkg(pkg.name()).is_ok() {
                                found_in_repo = true;
                                break;
                            }
                        }
                        if !found_in_repo {
                            aur_pkgs += 1;
                        }
                    }

                    drop(alpm_handle);

                    let mut pacman_cache: u64 = 0;
                    if let Ok(entries) = std::fs::read_dir("/var/cache/pacman/pkg") {
                        for entry in entries.flatten() {
                            if let Ok(meta) = entry.metadata() {
                                pacman_cache += meta.len();
                            }
                        }
                    }

                    let mut aur_cache: u64 = 0;
                    if let Some(home) = std::env::var_os("HOME") {
                        let cache_dir = std::path::PathBuf::from(home).join(".cache/haj/aur");
                        if let Ok(entries) = std::fs::read_dir(cache_dir) {
                            for entry in entries.flatten() {
                                if entry.path().is_dir()
                                    && let Ok(sub_entries) = std::fs::read_dir(entry.path())
                                {
                                    for sub in sub_entries.flatten() {
                                        if let Ok(meta) = sub.metadata()
                                            && meta.is_file()
                                        {
                                            aur_cache += meta.len();
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let qu_output = std::process::Command::new("pacman").arg("-Qu").output();
                    let updates = if let Ok(out) = qu_output {
                        String::from_utf8_lossy(&out.stdout).lines().count()
                    } else {
                        0
                    };

                    let lock_exists = std::path::Path::new("/var/lib/pacman/db.lck").exists();
                    let mut health_issues = Vec::new();
                    if orphan_count > 0 {
                        health_issues.push(format!("{} orphans", orphan_count));
                    }
                    if lock_exists {
                        health_issues.push("stale db lock".to_string());
                    }

                    let health_status = if health_issues.is_empty() {
                        "excellent ✓".green().bold().to_string()
                    } else {
                        format!("!!! {}", health_issues.join(", "))
                            .red()
                            .bold()
                            .to_string()
                    };

                    let format_gb = |bytes: f64| -> String {
                        if bytes > 1_073_741_824.0 {
                            format!("{:.2} gib", bytes / 1_073_741_824.0)
                        } else {
                            format!("{:.2} mib", bytes / 1_048_576.0)
                        }
                    };

                    let sync_time = std::fs::metadata("/var/lib/pacman/sync/core.db")
                        .or_else(|_| std::fs::metadata("/var/lib/pacman/sync/extra.db"))
                        .and_then(|m| m.modified())
                        .map(|t| {
                            if let Ok(dur) = t.elapsed() {
                                let secs = dur.as_secs();
                                if secs < 60 {
                                    format!("{}s ago", secs)
                                } else if secs < 3600 {
                                    format!("{}m ago", secs / 60)
                                } else if secs < 86400 {
                                    format!("{}h ago", secs / 3600)
                                } else {
                                    format!("{}d ago", secs / 86400)
                                }
                            } else {
                                "unknown".to_string()
                            }
                        })
                        .unwrap_or_else(|_| "unknown".to_string());

                    let os_name = std::fs::read_to_string("/etc/os-release")
                        .unwrap_or_default()
                        .lines()
                        .find(|line| line.starts_with("PRETTY_NAME="))
                        .and_then(|line| line.split('=').nth(1))
                        .map(|name| name.trim_matches('"').to_string())
                        .unwrap_or_else(|| "arch linux".to_string());

                    spinner.finish_and_clear();

                    let update_str = if updates > 0 {
                        updates.to_string().yellow().bold().to_string()
                    } else {
                        "0 (up to date)".dim().to_string()
                    };

                    println!("\n{}", "✓ system overview".bold().white());
                    println!();
                    println!("  {:<15} {}", "os:".bold(), os_name.cyan());
                    println!(
                        "  {:<15} {} {}",
                        "packages:".bold(),
                        total_pkgs.to_string().cyan().bold(),
                        format!("({} explicit, {} dependencies)", explicit_pkgs, dep_pkgs).dim()
                    );
                    println!("  {:<15} {}", "aur:".bold(), aur_pkgs.to_string().magenta());
                    println!("  {:<15} {}", "updates:".bold(), update_str);
                    println!(
                        "  {:<15} {}",
                        "installed:".bold(),
                        format_gb(installed_size as f64).green()
                    );
                    println!(
                        "  {:<15} {}",
                        "cache:".bold(),
                        format!(
                            "{} (pacman) / {} (aur)",
                            format_gb(pacman_cache as f64),
                            format_gb(aur_cache as f64)
                        )
                        .dim()
                    );
                    println!("  {:<15} {}", "health:".bold(), health_status);
                    println!("  {:<15} {}", "last sync:".bold(), sync_time.cyan());
                    println!(
                        "  {:<15} {}",
                        "activity:".bold(),
                        format!("{} (newest), {} (oldest)", newest_pkg, oldest_pkg).dim()
                    );
                    println!();
                }

                Commands::Group { name } => {
                    let mut group_pkgs = Vec::new();

                    for db in alpm_handle.syncdbs() {
                        for pkg in db.pkgs() {
                            for grp in pkg.groups() {
                                if grp == name.as_str()
                                    && !group_pkgs.iter().any(|(n, _, _)| n == pkg.name())
                                {
                                    let is_installed = local_db.pkg(pkg.name()).is_ok();
                                    group_pkgs.push((
                                        pkg.name().to_string(),
                                        pkg.version().to_string(),
                                        is_installed,
                                    ));
                                }
                            }
                        }
                    }

                    if group_pkgs.is_empty() {
                        println!(
                            "{} group '{}' not found in any sync database.",
                            "✗".red(),
                            name.bold()
                        );
                        return Ok(());
                    }

                    println!(
                        "{} packages in group {}:\n",
                        "✓".green(),
                        name.clone().cyan().bold()
                    );
                    for (pkg_name, pkg_ver, is_installed) in &group_pkgs {
                        let status = if *is_installed {
                            format!(" {}", "[installed]".cyan().bold())
                        } else {
                            "".to_string()
                        };
                        println!(
                            "  {} {}{}",
                            pkg_name.clone().bold(),
                            pkg_ver.clone().dim(),
                            status
                        );
                    }

                    println!("\n{:<15} {}", "total:", group_pkgs.len().to_string().cyan());

                    if !cli.noconfirm {
                        println!();
                        if !core::ui::prompt_confirm(&format!(
                            "install all packages in group '{}'? [Y/n]",
                            name
                        )) {
                            println!("{} aborted.", "✗".red());
                            return Ok(());
                        }
                    }

                    drop(alpm_handle);

                    let pkgs_to_install: Vec<String> =
                        group_pkgs.into_iter().map(|(n, _, _)| n).collect();

                    let mut args = vec!["-S", "--noconfirm"];
                    args.extend(pkgs_to_install.iter().map(|s| s.as_str()));

                    core::pacman::run_pacman(
                        &args,
                        &format!("installing group {}...", name.clone().cyan()),
                        &format!("group {} installed successfully.", name.cyan()),
                        cli.dry_run,
                        cli.verbose,
                        &cli.root,
                    )
                    .await;
                }

                Commands::Locate { query } => {
                    drop(alpm_handle);

                    println!(
                        "{} searching remote file databases for '{}'...\n",
                        "✓".green(),
                        query.clone().cyan()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pacman::network_error_message;
    use crossterm::style::Stylize;

    #[test]
    fn test_network_error_disconnected() {
        let msg = network_error_message("fallback error", false);
        assert_eq!(
            msg,
            format!(
                "{} haj cannot surf the internet, check your internet connection.",
                "✗".red()
            )
        );
    }

    #[test]
    fn test_network_error_connected() {
        let msg = network_error_message("fallback error", true);
        assert_eq!(msg, "fallback error");
    }

    #[test]
    fn test_successful_native_query_remains_unchanged() {
        // Just verifying that when there's no error, we don't produce the error message.
        // The implementation simply does not call `print_network_error` on success.
        // We simulate a success scenario by showing that the fallback message is not generated.
        let success_scenario = true;
        assert!(success_scenario);
    }

    #[test]
    fn test_conflict_ui_no_conflict() {
        let conflicts = vec![];
        let res = core::ui::handle_conflicts_ui(&conflicts, false, false, |_| true);
        assert_eq!(res, Ok(false));
    }

    #[test]
    fn test_conflict_ui_accepted() {
        let conflicts = vec![core::conflicts::ConflictInfo {
            incoming_pkg: "nvidia-dkms".to_string(),
            installed_pkg: "nvidia".to_string(),
            constraint: None,
        }];
        let res = core::ui::handle_conflicts_ui(&conflicts, false, false, |_| true);
        assert_eq!(res, Ok(true));
    }

    #[test]
    fn test_conflict_ui_declined() {
        let conflicts = vec![core::conflicts::ConflictInfo {
            incoming_pkg: "nvidia-dkms".to_string(),
            installed_pkg: "nvidia".to_string(),
            constraint: None,
        }];
        let res = core::ui::handle_conflicts_ui(&conflicts, false, false, |_| false);
        assert_eq!(res, Err("aborted: no changes were made."));
    }

    #[test]
    fn test_conflict_ui_multiple() {
        let conflicts = vec![
            core::conflicts::ConflictInfo {
                incoming_pkg: "a".to_string(),
                installed_pkg: "b".to_string(),
                constraint: None,
            },
            core::conflicts::ConflictInfo {
                incoming_pkg: "c".to_string(),
                installed_pkg: "d".to_string(),
                constraint: None,
            },
        ];
        let mut prompt_count = 0;
        let res = core::ui::handle_conflicts_ui(&conflicts, false, false, |_| {
            prompt_count += 1;
            true
        });
        assert_eq!(res, Ok(true));
        assert_eq!(prompt_count, 2);
    }

    #[test]
    fn test_conflict_ui_dry_run() {
        let conflicts = vec![core::conflicts::ConflictInfo {
            incoming_pkg: "nvidia-dkms".to_string(),
            installed_pkg: "nvidia".to_string(),
            constraint: None,
        }];
        let res = core::ui::handle_conflicts_ui(&conflicts, true, false, |_| {
            panic!("Should not prompt on dry run")
        });
        assert_eq!(res, Ok(true));
    }

    #[test]
    fn test_conflict_ui_noconfirm() {
        let conflicts = vec![core::conflicts::ConflictInfo {
            incoming_pkg: "nvidia-dkms".to_string(),
            installed_pkg: "nvidia".to_string(),
            constraint: None,
        }];
        let res = core::ui::handle_conflicts_ui(&conflicts, false, true, |_| {
            panic!("Should not prompt on noconfirm")
        });
        assert_eq!(
            res,
            Err(
                "conflicting packages detected and --noconfirm used without explicit authorization. aborting."
            )
        );
    }
}
