pub mod backend;
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
#[allow(unused_imports)]
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
        .map_err(|e| anyhow::anyhow!("failed to open lock file: {e}"))?;

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

    let cli = Cli::try_parse().unwrap_or_else(|e| {
        if e.kind() == clap::error::ErrorKind::DisplayHelp
            || e.kind() == clap::error::ErrorKind::DisplayVersion
        {
            println!(
                "\n{}",
                include_str!("../resources/title.txt")
                    .trim_end()
                    .cyan()
                    .bold()
            );
        }
        e.exit();
    });
    let mut cli = cli;
    let config = config::load_config();
    cli.aur = cli.aur || config.general.aur_only;
    cli.repo = cli.repo || config.general.repo_only;
    cli.verbose = cli.verbose || config.general.verbose;

    let active_command = cli.command.clone().unwrap_or_else(|| {
        use clap::CommandFactory;
        println!(
            "\n{}",
            include_str!("../resources/title.txt")
                .trim_end()
                .cyan()
                .bold()
        );
        let _ = Cli::command().print_help();
        std::process::exit(0);
    });

    let backend = match crate::backend::detect_backend() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{} {}", "error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    if matches!(active_command, Commands::Tui) {
        tui::run(backend.as_ref()).await?;
        return Ok(());
    }

    if let Commands::Completions { shell } = active_command {
        match shell {
            clap_complete::Shell::Bash => print!("{}", include_str!("completions/bash.sh")),
            clap_complete::Shell::Zsh => print!("{}", include_str!("completions/zsh.sh")),
            clap_complete::Shell::Fish => print!("{}", include_str!("completions/fish.sh")),
            _ => {
                use clap::CommandFactory;
                let mut cmd = Cli::command();
                let bin_name = cmd.get_name().to_string();
                clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
            }
        }
        return Ok(());
    }

    
    let backend_ref = backend.as_ref();

    if backend.name() == "pacman" {
        match active_command {
            Commands::Install { .. } | Commands::Interactive(_) | Commands::Search { .. } => {
                crate::core::pacman::check_and_offer_sync(&cli).await;
            }
            _ => {}
        }
    }

    match active_command {
        Commands::Tui | Commands::Completions { .. } => unreachable!(),

        Commands::Update => {
            if let Ok(plan) = backend_ref.build_update() {
                let _ = crate::core::process::execute_plan(&plan, "updating repositories...", "updated successfully.", cli.dry_run, cli.verbose).await;
            }
        }

        Commands::Install { packages } => {
            commands::process_installation(packages, backend_ref, &cli).await;
        }

        Commands::Interactive(queries) => {
            let query_str = queries.join(" ");
            println!("{} searching for '{}'...\n", "::".blue(), query_str.clone().bold());
            let results = backend_ref.search(&query_str).await.unwrap_or_default();
            
            if results.is_empty() {
                println!("no packages found.");
                return Ok(());
            }

            let (_, height) = crossterm::terminal::size().unwrap_or((80, 24));
            
            let mut output_str = String::new();
            for (i, pkg) in results.iter().enumerate() {
                let is_installed = pkg.is_installed;
                let status = if is_installed {
                    format!("{} {}", pkg.repo.clone().blue(), "[installed]".cyan().bold())
                } else {
                    pkg.repo.clone().blue().to_string()
                };
                
                let name_colored = if pkg.name == query_str {
                    pkg.name.clone().magenta().bold()
                } else {
                    pkg.name.clone().white().bold()
                };
                
                let idx_colored = (i + 1).to_string().cyan();
                let ver_colored = pkg.version.clone().dim();
                
                let line1 = format!("{:>3} {} {} {}", idx_colored, name_colored, ver_colored, status);
                
                let desc = pkg.desc.as_deref().unwrap_or("no description provided.");
                let line2 = format!("       {}", desc.dim());
                
                output_str.push_str(&line1);
                output_str.push('\n');
                output_str.push_str(&line2);
                output_str.push_str("\n\n");
            }
            
            if results.len() > (height as usize).saturating_sub(4) {
                use std::io::Write;
                if let Ok(mut child) = std::process::Command::new("less")
                    .arg("-R")
                    .arg("-F")
                    .arg("-X")
                    .stdin(std::process::Stdio::piped())
                    .spawn() 
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = write!(stdin, "{}", output_str);
                    }
                    let _ = child.wait();
                } else {
                    print!("{}", output_str);
                }
            } else {
                print!("{}", output_str);
            }
            
            print!(
                "\n{} enter packages to install (e.g., 1 2 3) [leave blank to abort]: ",
                "?".magenta().bold()
            );
            use std::io::Write;
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

            let pkgs_to_install = selections.into_iter().map(|idx| results[idx - 1].name.clone()).collect::<Vec<_>>();
            commands::process_installation(pkgs_to_install, backend_ref, &cli).await;
        }

        Commands::Remove { packages, .. } => {
            let pkgs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
            if let Ok(plan) = backend_ref.build_remove(&pkgs) {
                let _ = crate::core::process::execute_plan(&plan, "removing packages...", "removed successfully.", cli.dry_run, cli.verbose).await;
            }
        }

        Commands::Upgrade { .. } => {
            if let Ok(plan) = backend_ref.build_upgrade() {
                let _ = crate::core::process::execute_plan(&plan, "upgrading system...", "upgraded successfully.", cli.dry_run, cli.verbose).await;
            }
        }

        Commands::History { limit } => {
            if let Ok(logs) = backend_ref.get_history(limit).await {
                for log in logs {
                    let action = match log.action.as_str() {
                        "installed" | "install" => log.action.clone().green(),
                        "removed" | "remove" => log.action.clone().red(),
                        "upgraded" | "upgrade" => log.action.clone().yellow(),
                        _ => log.action.clone().cyan(),
                    };
                    println!("[{}] {} {} {}", log.timestamp.dim(), action, log.package.clone().bold(), log.version);
                }
            }
        }

        Commands::Downgrade { package, version } => {
            let plan = if let Some(v) = version {
                backend_ref.build_downgrade(&package, &v).ok()
            } else {
                if backend_ref.capabilities().supports_interactive_downgrade {
                    backend_ref.build_interactive_downgrade(&package).ok()
                } else {
                    println!("this backend requires a specific version for downgrade. use: haj downgrade <pkg> [version]");
                    None
                }
            };
            if let Some(plan) = plan {
                let _ = crate::core::process::execute_plan(&plan, "downgrading...", "downgraded.", cli.dry_run, cli.verbose).await;
            }
        }

        Commands::Clean { .. } => {
            if let Ok(plan) = backend_ref.build_clean() {
                let _ = crate::core::process::execute_plan(&plan, "cleaning...", "cleaned.", cli.dry_run, cli.verbose).await;
            }
        }

        Commands::Search { query } => {
            if let Ok(results) = backend_ref.search(&query).await {
                for pkg in results {
                    let is_installed = pkg.is_installed;
                    let status = if is_installed {
                        format!("{} {}", pkg.repo.clone().blue(), "[installed]".cyan().bold())
                    } else {
                        pkg.repo.clone().blue().to_string()
                    };
                    
                    let name_colored = if pkg.name == query {
                        pkg.name.clone().magenta().bold()
                    } else {
                        pkg.name.clone().white().bold()
                    };
                    
                    let ver_colored = pkg.version.clone().dim();
                    
                    println!("{} {} {}", name_colored, ver_colored, status);
                    let desc = pkg.desc.as_deref().unwrap_or("no description provided.");
                    println!("    {}\n", desc.dim());
                }
            }
        }

        Commands::Show { package } => {
            if let Ok(Some(info)) = backend_ref.info(&package).await {
                println!("Name: {}\nVersion: {}\nRepo: {}", info.name, info.version, info.repo);
            }
        }

        Commands::Orphan => {
            if let Ok(plan) = backend_ref.build_clean_orphans() {
                let _ = crate::core::process::execute_plan(&plan, "cleaning orphans...", "orphans cleaned.", cli.dry_run, cli.verbose).await;
            }
        }

        Commands::Owns { file_path } => {
            if let Ok(Some(pkg)) = backend_ref.owns_file(&file_path).await {
                println!("{} is owned by {}", file_path, pkg.name);
            }
        }

        Commands::Files { package } => {
            if let Ok(files) = backend_ref.list_files(&package).await {
                for file in files { println!("{}", file); }
            }
        }

        Commands::Fetch { packages } => {
            let pkgs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
            if let Ok(plan) = backend_ref.build_fetch(&pkgs) {
                let _ = crate::core::process::execute_plan(&plan, "fetching...", "fetched.", cli.dry_run, cli.verbose).await;
            }
        }

        Commands::Mark { package } => {
            if let Ok(plan) = backend_ref.build_mark_explicit(&package) {
                let _ = crate::core::process::execute_plan(&plan, "marking...", "marked.", cli.dry_run, cli.verbose).await;
            }
        }

        Commands::List { .. } => {
            if let Ok(pkgs) = backend_ref.list_installed().await {
                for pkg in pkgs { println!("{} {}", pkg.name, pkg.version); }
            }
        }

        Commands::Stats => {
            if let Ok(stats) = backend_ref.get_stats().await {
                println!("{}: {}", "explicit".green(), stats.explicit);
                println!("{}: {}", "dependencies".cyan(), stats.dependencies);
                println!("{}: {}", "total".bold(), stats.total);
            }
        }

        Commands::Group { name } => {
            if let Ok(pkgs) = backend_ref.list_group_or_meta(&name).await {
                for pkg in pkgs { println!("{}", pkg.name); }
            }
        }

        Commands::Locate { query } => {
            if let Ok(files) = backend_ref.locate_file(&query).await {
                for file in files { println!("{}", file); }
            }
        }
        
        _ => {
            println!("Not supported by generic backend yet.");
        }
    }

    Ok(())
}
