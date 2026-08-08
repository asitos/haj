use crate::cli::Cli;
use crate::core;
use crate::core::ui::prompt_confirm;
use crate::core::ui::{InstallChoice, handle_conflicts_ui, prompt_install};
use crate::ui;
use crossterm::style::Stylize;
pub async fn process_installation(packages: Vec<String>, alpm_handle: alpm::Alpm, cli: &Cli) {
    let mut native_pkgs = Vec::new();
    let mut aur_pkgs = Vec::new();

    for pkg in packages {
        let mut found_in_repo = false;

        if !cli.aur {
            for db in alpm_handle.syncdbs() {
                if db.pkg(pkg.as_str()).is_ok() {
                    found_in_repo = true;
                    break;
                }
            }
        }

        if found_in_repo || cli.repo {
            native_pkgs.push(pkg.clone());
        } else {
            let local_ver = alpm_handle
                .localdb()
                .pkg(pkg.as_str())
                .map(|p| p.version().to_string())
                .ok();
            aur_pkgs.push((pkg.clone(), local_ver));
        }
    }

    let mut native_summaries = Vec::new();
    let mut total_dl = 0.0;
    let mut total_inst = 0.0;

    if !native_pkgs.is_empty() {
        println!("{} resolving native dependencies...", "::".blue());
        match core::get_install_summaries(&alpm_handle, &native_pkgs) {
            Ok(summaries) => {
                for sum in &summaries {
                    total_dl += sum.download_size_mb;
                    total_inst += sum.install_size_mb;
                }
                native_summaries = summaries;
            }
            Err(e) => {
                core::pacman::print_network_error(&format!("{} {}", "✗".red(), e));
                native_pkgs.clear();
            }
        }
    }

    let mut resolved_aur_pkgs = Vec::new();
    let mut aur_conflicts_map = std::collections::HashMap::new();
    if !aur_pkgs.is_empty() {
        let check_spinner = ui::progress::spinner("querying aur...");

        let mut url = String::from("https://aur.archlinux.org/rpc/v5/info?");
        for (pkg, _) in &aur_pkgs {
            url.push_str(&format!("arg[]={}&", pkg));
        }

        if let Ok(response) = reqwest::get(&url).await
            && let Ok(json) = response.json::<serde_json::Value>().await
            && let Some(results) = json.get("results").and_then(|r| r.as_array())
        {
            check_spinner.finish_and_clear();

            for (pkg, local_ver) in aur_pkgs {
                if let Some(result) = results
                    .iter()
                    .find(|r| r.get("Name").and_then(|n| n.as_str()) == Some(&pkg))
                {
                    if let Some(conflicts_val) = result.get("Conflicts").and_then(|c| c.as_array())
                    {
                        let c_list: Vec<String> = conflicts_val
                            .iter()
                            .filter_map(|c| c.as_str().map(|s| s.to_string()))
                            .collect();
                        if !c_list.is_empty() {
                            aur_conflicts_map.insert(pkg.clone(), c_list);
                        }
                    }

                    if let Some(aur_ver) = result.get("Version").and_then(|v| v.as_str()) {
                        let mut is_update = false;
                        let mut skip = false;

                        if let Some(lv) = &local_ver {
                            if lv == aur_ver {
                                println!(
                                    "{} {} is up to date ({}).",
                                    "✓".green(),
                                    pkg.clone().magenta().bold(),
                                    aur_ver.dim()
                                );
                                skip = true;
                            } else {
                                is_update = true;
                            }
                        }

                        if !skip {
                            resolved_aur_pkgs.push((
                                pkg.clone(),
                                aur_ver.to_string(),
                                is_update,
                                local_ver.clone(),
                            ));
                        }
                    }
                } else {
                    println!(
                        "{} package '{}' not found on the aur.",
                        "✗".red(),
                        pkg.bold()
                    );
                }
            }
        } else {
            check_spinner.finish_and_clear();
            core::pacman::print_network_error(&format!("{} failed to query the aur.", "✗".red()));
        }
    }

    let conflicts =
        core::conflicts::detect_conflicts(&alpm_handle, &native_pkgs, &aur_conflicts_map);

    drop(alpm_handle);

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
                    local_ver.as_ref().unwrap().clone().red(),
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
            } else {
                if prompt_confirm(&prompt_msg) {
                    InstallChoice::Yes
                } else {
                    InstallChoice::No
                }
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
                    view_pkgbuilds(&aur_names).await;

                    prompt_msg = "proceed with installation? [Y/n]".to_string();
                }
            }
        }
    }

    if !native_pkgs.is_empty() {
        let mut args = vec!["-S", "--noconfirm"];
        if allow_conflict_removal {
            args.push("--ask=4");
        }
        args.extend(native_pkgs.iter().map(|s| s.as_str()));

        core::pacman::run_pacman(
            &args,
            "installing native packages...",
            "native packages installed successfully.",
            cli.dry_run,
            cli.verbose,
            &cli.root,
        )
        .await;
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

            match core::aur::build(&pkg, cli.verbose).await {
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

                    let mut pacman_args = vec!["-U", pkg_path.to_str().unwrap(), "--noconfirm"];
                    if allow_conflict_removal {
                        pacman_args.push("--ask=4");
                    }

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

pub async fn view_pkgbuilds(pkgs: &[String]) {
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
        let spinner = ui::progress::spinner(&format!(
            "fetching PKGBUILD for {}...",
            pkg.clone().magenta()
        ));
        let tmp_dir = format!("/tmp/haj_view_{}", pkg);

        let _ = std::fs::remove_dir_all(&tmp_dir);

        let clone_status = std::process::Command::new("git")
            .args([
                "clone",
                "--depth=1",
                "--quiet",
                &format!("https://aur.archlinux.org/{}.git", pkg),
                &tmp_dir,
            ])
            .status();

        spinner.finish_and_clear();

        if clone_status.is_ok_and(|s| s.success()) {
            let pkgbuild_path = format!("{}/PKGBUILD", tmp_dir);

            if std::path::Path::new(&pkgbuild_path).exists() {
                let parts: Vec<String> = editor.split_whitespace().map(|s| s.to_string()).collect();
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
