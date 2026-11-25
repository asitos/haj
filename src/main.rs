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
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

fn safe_println(line: &str) -> bool {
    let mut out = std::io::stdout().lock();
    match writeln!(out, "{line}") {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => false,
        Err(_) => false,
    }
}

fn print_search_entry(
    pkg: &crate::core::package::PackageModel,
    exact_query: &str,
    idx: Option<usize>,
) -> String {
    let repo_colored = if pkg.repo == "aur" {
        pkg.repo.clone().magenta()
    } else {
        pkg.repo.clone().blue()
    };
    let status = if pkg.is_installed {
        format!("{} {}", repo_colored, "[installed]".cyan().bold())
    } else {
        repo_colored.to_string()
    };
    let name_colored = if pkg.name == exact_query {
        pkg.name.clone().magenta().bold()
    } else {
        pkg.name.clone().white().bold()
    };
    let ver_colored = pkg.version.clone().dim();
    let desc = pkg.desc.as_deref().unwrap_or("no description provided.");

    if let Some(i) = idx {
        let idx_colored = (i + 1).to_string().cyan();
        format!(
            "{:>3} {} {} {}\n       {}\n\n",
            idx_colored,
            name_colored,
            ver_colored,
            status,
            desc.dim()
        )
    } else {
        format!(
            "{} {} {}\n    {}\n",
            name_colored,
            ver_colored,
            status,
            desc.dim()
        )
    }
}

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

    #[cfg(feature = "pacman")]
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

        Commands::Update => match backend_ref.build_update() {
            Ok(plan) => {
                let _ = crate::core::process::execute_plan(
                    &plan,
                    "updating repositories...",
                    "repositories synced successfully.",
                    cli.dry_run,
                    cli.verbose,
                )
                .await;
            }
            Err(e) => eprintln!("{} {}", "✗".red(), e),
        },

        Commands::Install { packages } => {
            commands::process_installation(packages, backend_ref, &cli).await;
        }

        Commands::Interactive(queries) => {
            let query_str = queries.join(" ");
            println!(
                "{} searching for '{}'...\n",
                "::".blue(),
                query_str.clone().bold()
            );
            let results = match backend_ref
                .search_filtered(&query_str, cli.repo, cli.aur)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{} {}", "✗".red(), e);
                    return Ok(());
                }
            };

            if results.is_empty() {
                println!("no packages found.");
                return Ok(());
            }

            let (_, height) = crossterm::terminal::size().unwrap_or((80, 24));
            let mut output_str = String::new();
            for (i, pkg) in results.iter().enumerate() {
                output_str.push_str(&print_search_entry(pkg, &query_str, Some(i)));
            }

            if results.len() > (height as usize).saturating_sub(4) {
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

            let pkgs_to_install = selections
                .into_iter()
                .map(|idx| results[idx - 1].name.clone())
                .collect::<Vec<_>>();
            commands::process_installation(pkgs_to_install, backend_ref, &cli).await;
        }

        Commands::Remove { packages } => {
            if !cli.noconfirm && !cli.dry_run {
                println!("{}", "targets to remove:".bold().white());
                for p in &packages {
                    println!("  {}", p.clone().magenta().bold());
                }
                println!();
                if !core::ui::prompt_confirm("proceed with removal? [Y/n]") {
                    println!("{} aborted.", "✗".red());
                    return Ok(());
                }
            }

            let pkgs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
            match backend_ref.build_remove(&pkgs) {
                Ok(plan) => {
                    let _ = crate::core::process::execute_plan(
                        &plan,
                        "removing packages...",
                        "packages removed successfully.",
                        cli.dry_run,
                        cli.verbose,
                    )
                    .await;
                }
                Err(e) => eprintln!("{} {}", "✗".red(), e),
            }
        }

        Commands::Upgrade { no_sync } => {
            if !no_sync
                && !cli.aur
                && let Ok(plan) = backend_ref.build_update()
            {
                let res = crate::core::process::execute_plan(
                    &plan,
                    "syncing package databases...",
                    "databases synced.",
                    cli.dry_run,
                    cli.verbose,
                )
                .await;
                if res.is_err() {
                    return Ok(());
                }
            }

            let native_updates = if cli.aur {
                Vec::new()
            } else {
                backend_ref.check_native_updates().await.unwrap_or_default()
            };

            let aur_updates = if cli.repo || !backend_ref.capabilities().supports_external {
                Vec::new()
            } else {
                backend_ref
                    .check_external_updates()
                    .await
                    .unwrap_or_default()
            };

            if native_updates.is_empty() && aur_updates.is_empty() {
                if cli.dry_run && !cli.aur {
                    if let Ok(plan) = backend_ref.build_upgrade() {
                        let _ = crate::core::process::execute_plan(
                            &plan,
                            "upgrading system...",
                            "system upgraded successfully.",
                            true,
                            cli.verbose,
                        )
                        .await;
                    }
                } else {
                    println!("{} system is fully up to date!", "✓".green());
                }
                return Ok(());
            }

            println!("{}", "available upgrades:".bold().white());
            for (name, old, new) in &native_updates {
                println!(
                    "  {:<30} {} -> {}",
                    name.clone().cyan().bold(),
                    old.clone().red(),
                    new.clone().green()
                );
            }
            for (name, old, new) in &aur_updates {
                println!(
                    "  {:<30} {} -> {}",
                    name.clone().magenta().bold(),
                    old.clone().red(),
                    new.clone().green()
                );
            }
            println!(
                "\n{:<15} {}",
                "total:",
                (native_updates.len() + aur_updates.len())
                    .to_string()
                    .cyan()
            );

            if !cli.noconfirm
                && !cli.dry_run
                && !core::ui::prompt_confirm("proceed with upgrade? [Y/n]")
            {
                println!("{} aborted.", "✗".red());
                return Ok(());
            }

            if !native_updates.is_empty()
                && let Ok(plan) = backend_ref.build_upgrade()
            {
                let _ = crate::core::process::execute_plan(
                    &plan,
                    "upgrading system packages...",
                    "system upgraded successfully.",
                    cli.dry_run,
                    cli.verbose,
                )
                .await;
            }

            for (name, _, new_ver) in aur_updates {
                if cli.dry_run {
                    println!(
                        "{} would build and update aur package: {} ({})",
                        "[dry run]".bold().yellow(),
                        name.magenta(),
                        new_ver.green()
                    );
                    continue;
                }
                match backend_ref.build_external_package(&name, cli.verbose).await {
                    Ok(pkg_path) => {
                        if let Ok(plan) = backend_ref.build_install_local(&pkg_path) {
                            let _ = crate::core::process::execute_plan(
                                &plan,
                                &format!("updating {}...", name.clone().magenta().bold()),
                                &format!("{} updated ({}).", name.magenta().bold(), new_ver.dim()),
                                false,
                                cli.verbose,
                            )
                            .await;
                        }
                    }
                    Err(e) => eprintln!("{} {}", "✗".red(), e),
                }
            }
        }

        Commands::History { limit } => match backend_ref.get_history(limit).await {
            Ok(logs) if logs.is_empty() => {
                println!("{} no transaction history found.", "::".blue());
            }
            Ok(logs) => {
                for log in logs {
                    let action = match log.action.as_str() {
                        "installed" | "install" => log.action.clone().green().bold(),
                        "removed" | "remove" => log.action.clone().red().bold(),
                        "upgraded" | "upgrade" => log.action.clone().yellow().bold(),
                        "downgraded" => log.action.clone().magenta().bold(),
                        _ => log.action.clone().cyan().bold(),
                    };
                    if !safe_println(&format!(
                        "[{}] {} {} {}",
                        log.timestamp.dim(),
                        action,
                        log.package.clone().bold(),
                        log.version.dim()
                    )) {
                        break;
                    }
                }
            }
            Err(e) => eprintln!("{} {}", "✗".red(), e),
        },

        Commands::Downgrade { package, version } => {
            if let Some(v) = version {
                match backend_ref.build_downgrade(&package, &v) {
                    Ok(plan) => {
                        let _ = crate::core::process::execute_plan(
                            &plan,
                            &format!("downgrading {} to {}...", package, v),
                            "package downgraded successfully.",
                            cli.dry_run,
                            cli.verbose,
                        )
                        .await;
                    }
                    Err(e) => println!("{} {}", "✗".red(), e),
                }
            } else {
                #[cfg(feature = "pacman")]
                if backend_ref.name() == "pacman" {
                    let candidates = crate::core::downgrade::list_candidates(&package);
                    if !candidates.is_empty() {
                        if cli.dry_run {
                            println!(
                                "{} cached versions available for {}:",
                                "::".blue(),
                                package.clone().magenta().bold()
                            );
                            for (_, ver) in &candidates {
                                println!("  - {}", ver.clone().green());
                            }
                            if let Ok(plan) = backend_ref.build_install_local(&candidates[0].0) {
                                let _ = crate::core::process::execute_plan(
                                    &plan,
                                    "downgrading...",
                                    "package downgraded successfully.",
                                    true,
                                    cli.verbose,
                                )
                                .await;
                            }
                        } else if let Some(path) =
                            crate::core::downgrade::select_downgrade_target(&package)
                            && let Ok(plan) = backend_ref.build_install_local(&path)
                        {
                            let _ = crate::core::process::execute_plan(
                                &plan,
                                "downgrading package...",
                                "package downgraded successfully.",
                                false,
                                cli.verbose,
                            )
                            .await;
                        }
                        return Ok(());
                    }

                    let has_downgrade_bin = std::process::Command::new("downgrade")
                        .arg("--version")
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .is_ok();

                    if has_downgrade_bin
                        && let Ok(plan) = backend_ref.build_interactive_downgrade(&package)
                    {
                        let _ = crate::core::process::execute_plan(
                            &plan,
                            "launching downgrade...",
                            "package downgraded successfully.",
                            cli.dry_run,
                            cli.verbose,
                        )
                        .await;
                    } else {
                        println!(
                            "{} no cached versions found for '{}' (and 'downgrade' is not installed).",
                            "✗".red(),
                            package.bold()
                        );
                    }
                    return Ok(());
                }

                if backend_ref.capabilities().supports_interactive_downgrade {
                    match backend_ref.build_interactive_downgrade(&package) {
                        Ok(plan) => {
                            let _ = crate::core::process::execute_plan(
                                &plan,
                                "downgrading...",
                                "package downgraded successfully.",
                                cli.dry_run,
                                cli.verbose,
                            )
                            .await;
                        }
                        Err(e) => println!("{} {}", "✗".red(), e),
                    }
                } else {
                    println!(
                        "this backend requires a specific version for downgrade. use: haj downgrade <pkg> [version]"
                    );
                }
            }
        }

        Commands::Clean { keep } => match backend_ref.build_clean_keep(keep) {
            Ok(plan) => {
                let res = crate::core::process::execute_plan(
                    &plan,
                    &format!("scrubbing package cache (keeping {keep})..."),
                    "cache scrubbed successfully.",
                    cli.dry_run,
                    cli.verbose,
                )
                .await;

                if res.is_ok()
                    && backend_ref.name() == "pacman"
                    && plan.executable == "paccache"
                    && !cli.dry_run
                    && let Ok(home) = std::env::var("HOME").map(std::path::PathBuf::from)
                {
                    let aur_cache = home.join(".cache/haj/aur");
                    if aur_cache.exists()
                        && let Ok(entries) = std::fs::read_dir(&aur_cache)
                    {
                        let keep_str = keep.to_string();
                        for entry in entries.filter_map(Result::ok) {
                            if entry.path().is_dir()
                                && let Some(path_str) = entry.path().to_str()
                            {
                                let _ = std::process::Command::new("paccache")
                                    .args(["-r", "-k", &keep_str, "-c", path_str])
                                    .output();
                            }
                        }
                    }
                }
            }
            Err(e) => println!("{} {}", "✗".red(), e),
        },

        Commands::Search { query } => {
            match backend_ref.search_filtered(&query, cli.repo, cli.aur).await {
                Ok(results) if results.is_empty() => {
                    println!(
                        "{} no packages found matching '{}'.",
                        "✗".red(),
                        query.bold()
                    );
                }
                Ok(results) => {
                    for pkg in results {
                        let line = print_search_entry(&pkg, &query, None);
                        if !safe_println(line.trim_end_matches('\n')) {
                            break;
                        }
                    }
                }
                Err(e) => eprintln!("{} {}", "✗".red(), e),
            }
        }

        Commands::Show { package } => match backend_ref.details(&package).await {
            Ok(Some(d)) => {
                let pkg = &d.package;
                let installed_tag = if pkg.is_installed {
                    format!(" {}", "[installed]".cyan().bold())
                } else {
                    String::new()
                };
                let repo_colored = if pkg.repo == "aur" || pkg.repo == "local/aur" {
                    pkg.repo.clone().magenta()
                } else {
                    pkg.repo.clone().blue()
                };

                println!(
                    "{} {} {} ({}){}",
                    "::".blue(),
                    pkg.name.clone().cyan().bold(),
                    pkg.version.clone().dim(),
                    repo_colored,
                    installed_tag
                );
                if let Some(desc) = &pkg.desc {
                    println!("   {}", desc.clone().italic());
                }
                if pkg.size_mb > 0.0 {
                    println!(
                        "\n   {:<15} {}",
                        "size:",
                        format!("{:.2} mb", pkg.size_mb).green()
                    );
                }
                if let Some(reason) = &d.install_reason {
                    println!("   {:<15} {}", "reason:", reason.clone().yellow());
                }
                if let Some(votes) = d.votes {
                    println!("   {:<15} {}", "votes:", votes.to_string().magenta());
                }

                if !d.depends.is_empty() {
                    println!("\n{}", "depends on:".bold().white());
                    for (i, dep) in d.depends.iter().enumerate() {
                        let prefix = if i == d.depends.len() - 1 {
                            "└─"
                        } else {
                            "├─"
                        };
                        println!("  {} {}", prefix.dim(), dep.clone().magenta());
                    }
                }

                if !d.required_by.is_empty() {
                    println!("\n{}", "required by:".bold().white());
                    for (i, req) in d.required_by.iter().enumerate() {
                        let prefix = if i == d.required_by.len() - 1 {
                            "└─"
                        } else {
                            "├─"
                        };
                        println!("  {} {}", prefix.dim(), req.clone().cyan());
                    }
                }

                if !d.opt_depends.is_empty() {
                    println!("\n{}", "optional deps:".bold().white());
                    for (i, opt) in d.opt_depends.iter().enumerate() {
                        let prefix = if i == d.opt_depends.len() - 1 {
                            "└─"
                        } else {
                            "├─"
                        };
                        println!("  {} {}", prefix.dim(), opt.clone().dim());
                    }
                }
                println!();
            }
            Ok(None) => {
                println!("{} package '{}' not found.", "✗".red(), package.bold());
            }
            Err(e) => eprintln!("{} {}", "✗".red(), e),
        },

        Commands::Orphan => match backend_ref.list_orphans().await {
            Ok(orphans) => {
                if orphans.is_empty() && backend_ref.name() == "pacman" {
                    println!(
                        "{} no orphaned packages found. system is clean!",
                        "✓".green()
                    );
                    return Ok(());
                }
                if !orphans.is_empty() {
                    println!("{}", "orphans found:".bold().white());
                    let mut total_mb = 0.0;
                    for pkg in &orphans {
                        println!(
                            "  {:<25} {} ({:.2} mb)",
                            pkg.name.clone().magenta(),
                            pkg.version.clone().dim(),
                            pkg.size_mb
                        );
                        total_mb += pkg.size_mb;
                    }
                    println!("\n{:<15} {:.2} mb\n", "wasted space:", total_mb);

                    if !cli.dry_run
                        && !cli.noconfirm
                        && !core::ui::prompt_confirm("remove all orphaned packages? [Y/n]")
                    {
                        println!("{} aborted.", "✗".red());
                        return Ok(());
                    }
                }

                match backend_ref.build_clean_orphans() {
                    Ok(plan) => {
                        let _ = crate::core::process::execute_plan(
                            &plan,
                            "cleaning orphans...",
                            "orphans cleaned.",
                            cli.dry_run,
                            cli.verbose,
                        )
                        .await;
                    }
                    Err(e) => println!("{} {}", "::".blue(), e),
                }
            }
            Err(e) => eprintln!("{} {}", "✗".red(), e),
        },

        Commands::Owns { file_path } => match backend_ref.owns_file(&file_path).await {
            Ok(Some(pkg)) => {
                println!(
                    "{} is owned by {} {}",
                    file_path,
                    pkg.name.cyan().bold(),
                    pkg.version.dim()
                );
            }
            Ok(None) => {
                println!("{} no package owns '{}'.", "✗".red(), file_path);
            }
            Err(e) => eprintln!("{} {}", "✗".red(), e),
        },

        Commands::Files { package } => match backend_ref.list_files(&package).await {
            Ok(files) => {
                for file in files {
                    if !safe_println(&file) {
                        break;
                    }
                }
            }
            Err(e) => println!("{} {}", "✗".red(), e),
        },

        Commands::Load { archive_path } => {
            match backend_ref.build_install_local(std::path::Path::new(&archive_path)) {
                Ok(plan) => {
                    let _ = crate::core::process::execute_plan(
                        &plan,
                        "loading package archive...",
                        "package loaded successfully.",
                        cli.dry_run,
                        cli.verbose,
                    )
                    .await;
                }
                Err(e) => println!("{} {}", "✗".red(), e),
            }
        }

        Commands::Fetch { packages } => {
            let pkgs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
            match backend_ref.build_fetch(&pkgs) {
                Ok(plan) => {
                    let _ = crate::core::process::execute_plan(
                        &plan,
                        "fetching packages...",
                        "packages fetched successfully.",
                        cli.dry_run,
                        cli.verbose,
                    )
                    .await;
                }
                Err(e) => println!("{} {}", "✗".red(), e),
            }
        }

        Commands::Mark { package } => match backend_ref.toggle_mark(&package).await {
            Ok((plan, new_state)) => {
                let _ = crate::core::process::execute_plan(
                    &plan,
                    "updating database records...",
                    &format!("marked {package} as {new_state}."),
                    cli.dry_run,
                    cli.verbose,
                )
                .await;
            }
            Err(e) => println!("{} {}", "✗".red(), e),
        },

        Commands::List {
            explicit,
            deps,
            foreign,
        } => {
            match backend_ref
                .list_installed_filtered(explicit, deps, foreign)
                .await
            {
                Ok(pkgs) => {
                    for pkg in &pkgs {
                        let name_colored = if pkg.repo == "aur" {
                            pkg.name.clone().magenta()
                        } else {
                            pkg.name.clone().cyan()
                        };
                        let repo_colored = if pkg.repo == "aur" {
                            pkg.repo.clone().magenta()
                        } else {
                            pkg.repo.clone().blue()
                        };
                        if !safe_println(&format!(
                            "  {:<35} {:<25} {}",
                            name_colored,
                            pkg.version.clone().dim(),
                            repo_colored
                        )) {
                            break;
                        }
                    }
                    let _ = safe_println(&format!(
                        "\n{} {} packages listed.",
                        "✓".green(),
                        pkgs.len().to_string().bold()
                    ));
                }
                Err(e) => eprintln!("{} {}", "✗".red(), e),
            }
        }

        Commands::Stats => match backend_ref.get_stats().await {
            Ok(stats) => {
                let orphans = backend_ref.get_orphan_count().await.unwrap_or(0);
                println!("{}: {}", "explicit".green(), stats.explicit);
                println!("{}: {}", "dependencies".cyan(), stats.dependencies);
                println!("{}: {}", "total".bold(), stats.total);
                println!("{}: {}", "orphans".yellow(), orphans);
            }
            Err(e) => eprintln!("{} {}", "✗".red(), e),
        },

        Commands::Group { name } => match backend_ref.list_group_or_meta(&name).await {
            Ok(pkgs) if pkgs.is_empty() => {
                println!("{} group '{}' not found.", "✗".red(), name.bold());
            }
            Ok(pkgs) => {
                println!(
                    "{} packages in group {}:\n",
                    "✓".green(),
                    name.clone().cyan().bold()
                );
                for pkg in &pkgs {
                    let status = if pkg.is_installed {
                        format!(" {}", "[installed]".cyan().bold())
                    } else {
                        String::new()
                    };
                    println!(
                        "  {} {}{}",
                        pkg.name.clone().bold(),
                        pkg.version.clone().dim(),
                        status
                    );
                }
                println!("\n{:<15} {}\n", "total:", pkgs.len().to_string().cyan());

                if !cli.dry_run
                    && !cli.noconfirm
                    && !core::ui::prompt_confirm(&format!(
                        "install all packages in group '{name}'? [Y/n]"
                    ))
                {
                    println!("{} aborted.", "✗".red());
                    return Ok(());
                }

                let names: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
                if let Ok(plan) = backend_ref.build_install(&names) {
                    let _ = crate::core::process::execute_plan(
                        &plan,
                        &format!("installing group {}...", name.clone().cyan()),
                        &format!("group {} installed successfully.", name.cyan()),
                        cli.dry_run,
                        cli.verbose,
                    )
                    .await;
                }
            }
            Err(e) => println!("{} {}", "✗".red(), e),
        },

        Commands::Locate { query } => match backend_ref.locate_file(&query).await {
            Ok(files) if files.is_empty() => {
                println!(
                    "{} no results found. (you may need to sync file databases with 'sudo pacman -Fy')",
                    "✗".red()
                );
            }
            Ok(files) => {
                for file in files {
                    if !safe_println(&file) {
                        break;
                    }
                }
            }
            Err(e) => eprintln!("{} {}", "✗".red(), e),
        },

        Commands::Diff => {
            #[cfg(feature = "pacman")]
            if backend_ref.name() == "pacman" {
                if cli.dry_run {
                    println!(
                        "{} no system changes will be made.",
                        "[dry run]".bold().yellow()
                    );
                    println!("{} would execute: sudo pacdiff", "→".cyan());
                } else if let Err(e) = crate::core::manage_pacnew_files() {
                    eprintln!("{} {}", "error:".red(), e);
                }
                return Ok(());
            }
            println!("Not supported by this backend.");
        }

        Commands::Pkgbuild { package } => {
            if backend_ref.capabilities().supports_external {
                if cli.dry_run {
                    println!(
                        "{} would fetch and inspect PKGBUILD for {}",
                        "[dry run]".bold().yellow(),
                        package.magenta()
                    );
                } else {
                    commands::view_pkgbuilds(&[package]);
                }
            } else {
                println!("PKGBUILD inspection is not supported by this backend.");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::backend::traits::ConflictInfo;
    use crate::core::ui::handle_conflicts_ui;
    use crossterm::style::Stylize;

    #[test]
    fn test_conflict_ui_no_conflict() {
        let conflicts = vec![];
        let res = handle_conflicts_ui(&conflicts, false, false, |_| true);
        assert_eq!(res, Ok(false));
    }

    #[test]
    fn test_conflict_ui_accepted() {
        let conflicts = vec![ConflictInfo {
            incoming_pkg: "nvidia-dkms".to_string(),
            installed_pkg: "nvidia".to_string(),
            constraint: None,
        }];
        let res = handle_conflicts_ui(&conflicts, false, false, |_| true);
        assert_eq!(res, Ok(true));
    }

    #[test]
    fn test_conflict_ui_declined() {
        let conflicts = vec![ConflictInfo {
            incoming_pkg: "nvidia-dkms".to_string(),
            installed_pkg: "nvidia".to_string(),
            constraint: None,
        }];
        let res = handle_conflicts_ui(&conflicts, false, false, |_| false);
        assert_eq!(res, Err("aborted: no changes were made."));
    }

    #[test]
    fn test_conflict_ui_dry_run() {
        let conflicts = vec![ConflictInfo {
            incoming_pkg: "nvidia-dkms".to_string(),
            installed_pkg: "nvidia".to_string(),
            constraint: None,
        }];
        let res = handle_conflicts_ui(&conflicts, true, false, |_| {
            panic!("Should not prompt on dry run")
        });
        assert_eq!(res, Ok(true));
    }

    #[test]
    fn test_conflict_ui_noconfirm() {
        let conflicts = vec![ConflictInfo {
            incoming_pkg: "nvidia-dkms".to_string(),
            installed_pkg: "nvidia".to_string(),
            constraint: None,
        }];
        let res = handle_conflicts_ui(&conflicts, false, true, |_| {
            panic!("Should not prompt on noconfirm")
        });
        assert_eq!(
            res,
            Err(
                "conflicting packages detected and --noconfirm used without explicit authorization. aborting."
            )
        );
    }

    #[test]
    fn test_aur_response_deserialization() {
        let json = r#"{
            "resultcount": 1,
            "results": [{
                "Name": "yay-bin",
                "Version": "13.1.0-1",
                "Description": "Yet another yogurt.",
                "NumVotes": 371,
                "Conflicts": ["yay"],
                "Depends": ["pacman>6.1", "git"],
                "OptDepends": ["sudo"]
            }]
        }"#;
        let parsed: crate::core::aur::AurResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].name, "yay-bin");
        assert_eq!(parsed.results[0].num_votes, 371);
        assert_eq!(parsed.results[0].depends, vec!["pacman>6.1", "git"]);
        let _ = "ok".green();
    }
}
