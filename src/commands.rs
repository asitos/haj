use crate::cli::Cli;
use crate::core;
use crate::core::ui::prompt_confirm;
use crate::core::ui::{InstallChoice, handle_conflicts_ui, prompt_install};
use crate::ui;
use crossterm::style::Stylize;
pub async fn process_installation(
    packages: Vec<String>,
    backend: &dyn crate::backend::traits::PackageManager,
    cli: &Cli,
) {
    let mut native_pkgs: Vec<String> = Vec::new();
    let mut aur_pkgs: Vec<String> = Vec::new();

    for pkg in &packages {
        if cli.repo || backend.info(pkg).await.is_ok() {
            native_pkgs.push(pkg.clone());
        } else if backend.capabilities().supports_aur {
            aur_pkgs.push(pkg.clone());
        } else {
            println!("{} package '{}' not found", "✗".red(), pkg.clone().bold());
            return;
        }
    }

    let mut native_summaries = Vec::new();
    let mut total_dl = 0.0;
    let mut total_inst = 0.0;

    if !native_pkgs.is_empty() {
        println!("{} resolving native dependencies...", "::".blue());
        let native_pkgs_str: Vec<&str> = native_pkgs.iter().map(|s| s.as_str()).collect();
        match backend.get_install_summaries(&native_pkgs_str).await {
            Ok(summaries) => {
                for sum in &summaries {
                    total_dl += sum.download_size_mb;
                    total_inst += sum.install_size_mb;
                }
                native_summaries = summaries;
            }
            Err(e) => {
                println!("{} {}", "✗".red(), e);
                native_pkgs.clear();
            }
        }
    }

    let mut resolved_aur_pkgs = Vec::new();
    if !aur_pkgs.is_empty() {
        let aur_pkgs_str: Vec<&str> = aur_pkgs.iter().map(|s| s.as_str()).collect();
        match backend.resolve_aur(&aur_pkgs_str).await {
            Ok(resolution) => resolved_aur_pkgs = resolution.packages,
            Err(e) => {
                println!("{} failed to resolve aur packages: {}", "✗".red(), e);
                return;
            }
        }
    }

    let native_pkgs_str: Vec<&str> = native_pkgs.iter().map(|s| s.as_str()).collect();
    let conflicts = backend
        .detect_conflicts(&native_pkgs_str)
        .await
        .unwrap_or_default();

    let allow_conflict_removal =
        match handle_conflicts_ui(&conflicts, cli.dry_run, cli.noconfirm, prompt_confirm) {
            Ok(allowed) => allowed,
            Err(e) => {
                println!("{} {}", "✗".red(), e);
                std::process::exit(1);
            }
        };

    if native_summaries.is_empty() && resolved_aur_pkgs.is_empty() {
        println!("{} nothing to do.", "✓".green());
        return;
    }

    println!("\n{}", "targets:".bold().white());

    if !native_summaries.is_empty() {
        println!("  {}", "native repositories:".dim());
        for sum in &native_summaries {
            println!(
                "    {:<25} {}",
                sum.name.clone().cyan().bold(),
                sum.version.clone().dim()
            );
        }
    }

    if !resolved_aur_pkgs.is_empty() {
        println!("  {}", "arch user repository:".dim());
        for (pkg, aur_ver, is_update, local_ver) in &resolved_aur_pkgs {
            if *is_update {
                println!(
                    "    {:<25} {} -> {}",
                    pkg.clone().magenta().bold(),
                    local_ver.as_deref().unwrap_or_default().red(),
                    aur_ver.clone().green()
                );
            } else {
                println!(
                    "    {:<25} {}",
                    pkg.clone().magenta().bold(),
                    aur_ver.clone().green()
                );
            }
        }
    }

    if total_dl > 0.0 || total_inst > 0.0 {
        println!("\n{:<15} {:.2} mb", "download:", total_dl);
        println!("{:<15} {:.2} mb", "disk usage:", total_inst);
    } else {
        println!();
    }

    if !cli.noconfirm {
        let has_aur = !resolved_aur_pkgs.is_empty();
        let mut prompt_msg = if has_aur {
            "proceed with installation? [Y/n/v] (v = view PKGBUILDs)".to_string()
        } else {
            "proceed with installation? [Y/n]".to_string()
        };

        loop {
            let choice = if has_aur {
                prompt_install(&prompt_msg)
            } else if prompt_confirm(&prompt_msg) {
                InstallChoice::Yes
            } else {
                InstallChoice::No
            };

            match choice {
                InstallChoice::Yes => break,
                InstallChoice::No => {
                    println!("{} aborted.", "✗".red());
                    return;
                }
                InstallChoice::View => {
                    let aur_names: Vec<String> = resolved_aur_pkgs
                        .iter()
                        .map(|(pkg, _, _, _)| pkg.clone())
                        .collect();
                    view_pkgbuilds(&aur_names);

                    prompt_msg = "proceed with installation? [Y/n]".to_string();
                }
            }
        }
    }

    if !native_pkgs.is_empty() {
        let native_pkgs_str: Vec<&str> = native_pkgs.iter().map(|s| s.as_str()).collect();
        if let Ok(mut plan) = backend.build_install(&native_pkgs_str) {
            if allow_conflict_removal {
                plan.args.push("--ask=4".to_string());
            }
            let _ = crate::core::process::execute_plan(
                &plan,
                "installing native packages...",
                "native packages installed successfully.",
                cli.dry_run,
                cli.verbose,
            )
            .await;
        }
    }

    if !resolved_aur_pkgs.is_empty() {
        for (pkg, aur_ver, is_update, _) in resolved_aur_pkgs {
            if cli.dry_run {
                println!(
                    "{} would build and install aur package: {}",
                    "[dry run]".bold().yellow(),
                    pkg.magenta()
                );
                continue;
            }

            println!(
                "\n{} preparing {} ({})...",
                "::".blue(),
                pkg.clone().magenta().bold(),
                aur_ver.clone().green()
            );

            match backend.build_aur_package(&pkg, cli.verbose).await {
                Ok(pkg_path) => {
                    let spinner_msg = if is_update {
                        format!("updating built package {}...", pkg.clone().magenta().bold())
                    } else {
                        format!(
                            "installing built package {}...",
                            pkg.clone().magenta().bold()
                        )
                    };
                    let success_msg = if is_update {
                        format!(
                            "{} updated successfully ({}).",
                            pkg.magenta().bold(),
                            aur_ver.dim()
                        )
                    } else {
                        format!(
                            "{} installed successfully ({}).",
                            pkg.magenta().bold(),
                            aur_ver.dim()
                        )
                    };
                    if let Ok(mut plan) = backend.build_install_local(&pkg_path) {
                        if allow_conflict_removal {
                            plan.args.push("--ask=4".to_string());
                        }
                        let _ = crate::core::process::execute_plan(
                            &plan,
                            &spinner_msg,
                            &success_msg,
                            cli.dry_run,
                            cli.verbose,
                        )
                        .await;
                        println!("{}", success_msg.green());
                    }
                }
                Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
            }
        }
    }
}

pub fn view_pkgbuilds(pkgs: &[String]) {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| {
            if std::process::Command::new("nvim")
                .arg("--version")
                .output()
                .is_ok()
            {
                "nvim".to_string()
            } else if std::process::Command::new("vim")
                .arg("--version")
                .output()
                .is_ok()
            {
                "vim".to_string()
            } else if std::process::Command::new("nano")
                .arg("--version")
                .output()
                .is_ok()
            {
                "nano".to_string()
            } else {
                "less".to_string()
            }
        });

    for pkg in pkgs {
        let spinner = ui::spinner(&format!(
            "fetching PKGBUILD for {}...",
            pkg.clone().magenta()
        ));
        let tmp_dir = format!("/tmp/haj_view_{pkg}");

        let _ = std::fs::remove_dir_all(&tmp_dir);

        let clone_status = std::process::Command::new("git")
            .args([
                "clone",
                "--depth=1",
                "--quiet",
                &format!("https://aur.archlinux.org/{pkg}.git"),
                &tmp_dir,
            ])
            .status();

        spinner.finish_and_clear();

        if clone_status.is_ok_and(|s| s.success()) {
            let pkgbuild_path = format!("{tmp_dir}/PKGBUILD");

            if std::path::Path::new(&pkgbuild_path).exists() {
                let parts: Vec<String> = editor
                    .split_whitespace()
                    .map(std::string::ToString::to_string)
                    .collect();
                if !parts.is_empty() {
                    let exec = &parts[0];
                    let mut cmd = std::process::Command::new(exec);

                    if parts.len() > 1 {
                        cmd.args(&parts[1..]);
                    }

                    let exec_lower = exec.to_lowercase();
                    if exec_lower.contains("nvim")
                        || exec_lower.contains("vim")
                        || exec_lower.contains("vi")
                    {
                        cmd.arg("-R");
                    } else if exec_lower.contains("nano") {
                        cmd.arg("-v");
                    }

                    cmd.arg(&pkgbuild_path);
                    let _ = cmd.status();
                }
            } else {
                println!(
                    "{} PKGBUILD not found in repository for {}.",
                    "✗".red(),
                    pkg.clone().bold()
                );
            }
        } else {
            println!(
                "{} failed to fetch repository for {}.",
                "✗".red(),
                pkg.clone().bold()
            );
        }

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
