mod cli;
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

fn prompt_confirm(msg: &str) -> bool {
    print!("{} {} ", "?".magenta().bold(), msg.bold());
    let _ = std::io::stdout().flush();

    if crossterm::terminal::enable_raw_mode().is_ok() {
        let result;
        loop {
            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && key.code == crossterm::event::KeyCode::Char('c')
                {
                    result = false;
                    break;
                }
                match key.code {
                    crossterm::event::KeyCode::Char('y')
                    | crossterm::event::KeyCode::Char('Y')
                    | crossterm::event::KeyCode::Enter => {
                        result = true;
                        break;
                    }
                    crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Char('N') => {
                        result = false;
                        break;
                    }
                    _ => continue,
                }
            }
        }
        let _ = crossterm::terminal::disable_raw_mode();
        println!("{}", if result { "Y" } else { "n" });
        result
    } else {
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        let input = input.trim().to_lowercase();
        input.is_empty() || input == "y"
    }
}

pub enum InstallChoice {
    Yes,
    No,
    View,
}

fn prompt_install(msg: &str) -> InstallChoice {
    print!("{} {} ", "?".magenta().bold(), msg.bold());
    let _ = std::io::stdout().flush();

    if crossterm::terminal::enable_raw_mode().is_ok() {
        let result;
        loop {
            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && key.code == crossterm::event::KeyCode::Char('c')
                {
                    result = InstallChoice::No;
                    break;
                }
                match key.code {
                    crossterm::event::KeyCode::Char('y')
                    | crossterm::event::KeyCode::Char('Y')
                    | crossterm::event::KeyCode::Enter => {
                        result = InstallChoice::Yes;
                        break;
                    }
                    crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Char('N') => {
                        result = InstallChoice::No;
                        break;
                    }
                    crossterm::event::KeyCode::Char('v') | crossterm::event::KeyCode::Char('V') => {
                        result = InstallChoice::View;
                        break;
                    }
                    _ => continue,
                }
            }
        }
        let _ = crossterm::terminal::disable_raw_mode();
        let display_str = match result {
            InstallChoice::Yes => "Y",
            InstallChoice::No => "n",
            InstallChoice::View => "v",
        };
        println!("{}", display_str);
        result
    } else {
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        let input = input.trim().to_lowercase();
        if input == "v" {
            InstallChoice::View
        } else if input.is_empty() || input == "y" {
            InstallChoice::Yes
        } else {
            InstallChoice::No
        }
    }
}

async fn display_arch_news() {
    let spinner = ui::progress::spinner("checking arch linux news...");
    let url = "https://archlinux.org/feeds/news/";

    if let Ok(response) = reqwest::get(url).await {
        if let Ok(xml) = response.text().await {
            spinner.finish_and_clear();

            if let Some(item_start) = xml.find("<item>") {
                let item_str = &xml[item_start..];

                if let (Some(t_start), Some(t_end)) =
                    (item_str.find("<title>"), item_str.find("</title>"))
                {
                    let title = &item_str[t_start + 7..t_end];

                    if let (Some(d_start), Some(d_end)) =
                        (item_str.find("<pubDate>"), item_str.find("</pubDate>"))
                    {
                        let date_str = &item_str[d_start + 9..d_end];

                        if let Ok(pub_date) = chrono::DateTime::parse_from_rfc2822(date_str) {
                            let now = chrono::Utc::now();
                            if now
                                .signed_duration_since(pub_date.with_timezone(&chrono::Utc))
                                .num_days()
                                <= 7
                            {
                                println!(
                                    "\n{} {}\n  {} {}\n  {}\n",
                                    "!!!".red().bold(),
                                    "ACTION REQUIRED: recent Arch Linux news".red().bold(),
                                    "headline:".dim(),
                                    title.yellow().bold(),
                                    "haj requests you to read the news before upgrading.".dim()
                                );
                            }
                        }
                    }
                }
            }
        } else {
            spinner.finish_and_clear();
        }
    } else {
        spinner.finish_and_clear();
    }
}

async fn view_pkgbuilds(pkgs: &[String]) {
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

async fn run_pacman(
    args: &[&str],
    spinner_msg: &str,
    success_msg: &str,
    is_dry_run: bool,
    is_verbose: bool,
    root: &Option<String>,
) {
    if is_dry_run {
        println!(
            "{} no system changes will be made.",
            "[dry run]".bold().yellow()
        );
        let arrow = "→".cyan();
        let cmd = args.join(" ");
        let root_arg = root
            .as_ref()
            .map_or(String::new(), |r| format!("--root {} ", r));
        println!("{arrow} would execute: sudo pacman {root_arg}{cmd}");
        return;
    }

    if let Err(e) = core::escalate::ensure_sudo().await {
        println!("{} {}", "✗".red(), e);
        return;
    }

    let is_root = unsafe { libc::geteuid() == 0 };

    let mut child_cmd = if is_root {
        tokio::process::Command::new("pacman")
    } else {
        let mut c = tokio::process::Command::new("sudo");
        c.arg("pacman");
        c
    };

    let mut final_args = Vec::new();
    if let Some(r) = root {
        final_args.push("--root");
        final_args.push(r.as_str());
    }
    final_args.extend_from_slice(args);

    if is_verbose {
        println!(
            "{} [verbose] executing: pacman {}",
            "::".blue(),
            args.join(" ")
        );
        let mut child = child_cmd
            .args(args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("failed to spawn pacman");

        let status = child.wait().await;
        if status.is_ok_and(|s| s.success()) {
            println!("{} {}", "✓".green(), success_msg);
        } else {
            println!("{} operation failed.", "✗".red());
        }
        return;
    }

    child_cmd.arg("--color=never");

    let mut spinner = ui::progress::spinner(spinner_msg);
    let mut last_spinner_msg = spinner_msg.to_string();
    let mut context_buffer: Vec<String> = Vec::new();
    let mut in_hook_phase = false;

    let mut child = child_cmd
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn pacman");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let err_handle = tokio::spawn(async move {
        let mut err_str = String::new();
        let mut buf = [0u8; 1024];
        while let Ok(n) = tokio::io::AsyncReadExt::read(&mut stderr, &mut buf).await {
            if n == 0 {
                break;
            }
            err_str.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
        err_str
    });

    let mut buf = [0u8; 128];
    let mut current_line = String::new();
    let mut hook_alerts = Vec::new();

    let is_noconfirm = args.contains(&"--noconfirm");

    while let Ok(n) = tokio::io::AsyncReadExt::read(&mut stdout, &mut buf).await {
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buf[..n]);

        for c in chunk.chars() {
            if c == '\n' || c == '\r' {
                let clean = current_line.trim();
                if clean.is_empty() {
                    current_line.clear();
                    continue;
                }

                let mut matched_spinner = true;
                let lower_clean = clean.to_lowercase();

                if (lower_clean.contains("error")
                    || lower_clean.contains("warning")
                    || lower_clean.contains("failed"))
                    && !hook_alerts.contains(&clean.to_string())
                {
                    hook_alerts.push(clean.to_string());
                }

                if clean.contains("resolving dependencies")
                    || clean.contains("conflicting packages")
                {
                    last_spinner_msg = format!(
                        "{} {}",
                        ":3c".yellow(),
                        "resolving package dependencies...".bold()
                    );
                    spinner.set_message(last_spinner_msg.clone());
                } else if clean.contains("checking keys")
                    || clean.contains("checking package integrity")
                    || clean.contains("loading package files")
                {
                    last_spinner_msg = format!(
                        "{} {}",
                        ":O".yellow(),
                        "verifying package integrity...".bold()
                    );
                    spinner.set_message(last_spinner_msg.clone());
                } else if clean.contains("Retrieving packages") || clean.contains("downloading") {
                    last_spinner_msg = format!("  {}", "downloading packages...".dim());
                    spinner.set_message(last_spinner_msg.clone());
                } else if clean.starts_with('(') && clean.contains(") upgrading")
                    || clean.starts_with('(') && clean.contains(") installing")
                {
                    if let Some(idx_end) = clean.find(')') {
                        let counter = &clean[..=idx_end];
                        let action = if clean.contains("installing") {
                            "installing"
                        } else {
                            "upgrading"
                        };
                        last_spinner_msg = format!(
                            "{} {} packages {}...",
                            ":p".yellow(),
                            counter.cyan().bold(),
                            action
                        );
                        spinner.set_message(last_spinner_msg.clone());
                    }
                } else if clean.contains("Running pre-transaction hooks")
                    || clean.contains("Running post-transaction hooks")
                {
                    in_hook_phase = true;
                    last_spinner_msg =
                        format!("{} {}", ":v".yellow(), "running system hooks...".bold());
                    spinner.set_message(last_spinner_msg.clone());
                } else if in_hook_phase {
                    if clean.starts_with("==> Building image")
                        || clean.starts_with("==> Install DKMS")
                    {
                        last_spinner_msg = format!("{}    {}", ":3".yellow(), clean.dim());
                        spinner.set_message(last_spinner_msg.clone());
                    } else if (lower_clean.contains("missing") || lower_clean.contains("not found"))
                        && !hook_alerts.contains(&clean.to_string())
                    {
                        hook_alerts.push(clean.to_string());
                    }
                } else if clean.starts_with("::") {
                    last_spinner_msg = clean.replace("::", "→").cyan().bold().to_string();
                    spinner.set_message(last_spinner_msg.clone());
                    context_buffer.push(current_line.clone());
                } else {
                    matched_spinner = false;
                }

                if !matched_spinner && !in_hook_phase {
                    context_buffer.push(current_line.clone());
                    if context_buffer.len() > 15 {
                        context_buffer.remove(0);
                    }
                }

                current_line.clear();
            } else {
                current_line.push(c);
                let lower = current_line.to_lowercase();
                let trimmed_lower = lower.trim_end();

                let is_yn = trimmed_lower.ends_with("[y/n]");
                let is_choice = trimmed_lower.ends_with("):");

                if (is_yn || is_choice) && !is_noconfirm {
                    spinner.finish_and_clear();

                    if !context_buffer.is_empty() {
                        for line in &context_buffer {
                            println!("  {}", line.clone().dim());
                        }
                        context_buffer.clear();
                    }

                    use std::io::Write;
                    print!("{} {} ", "?".magenta().bold(), current_line.trim().bold());
                    let _ = std::io::stdout().flush();

                    let is_yn_prompt = is_yn;
                    let user_input = tokio::task::spawn_blocking(move || {
                        if crossterm::terminal::enable_raw_mode().is_ok() {
                            let mut result = String::new();
                            loop {
                                if let Ok(crossterm::event::Event::Key(key)) =
                                    crossterm::event::read()
                                {
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL)
                                        && key.code == crossterm::event::KeyCode::Char('c')
                                    {
                                        result = "n\n".to_string();
                                        println!("^C");
                                        break;
                                    }
                                    match key.code {
                                        crossterm::event::KeyCode::Enter => {
                                            result.push('\n');
                                            println!();
                                            break;
                                        }
                                        crossterm::event::KeyCode::Backspace => {
                                            if !is_yn_prompt && !result.is_empty() {
                                                result.pop();
                                                print!("\x08 \x08");
                                                let _ = std::io::stdout().flush();
                                            }
                                        }
                                        crossterm::event::KeyCode::Char(c) => {
                                            result.push(c);
                                            print!("{}", c);
                                            let _ = std::io::stdout().flush();

                                            if is_yn_prompt {
                                                result.push('\n');
                                                println!();
                                                break;
                                            }
                                        }
                                        _ => continue,
                                    }
                                }
                            }
                            let _ = crossterm::terminal::disable_raw_mode();
                            result
                        } else {
                            let mut input = String::new();
                            let _ = std::io::stdin().read_line(&mut input);
                            input
                        }
                    })
                    .await
                    .unwrap_or_else(|_| "\n".to_string());

                    let _ = tokio::io::AsyncWriteExt::write_all(&mut stdin, user_input.as_bytes())
                        .await;
                    let _ = tokio::io::AsyncWriteExt::flush(&mut stdin).await;

                    current_line.clear();

                    println!();
                    spinner = ui::progress::spinner(&last_spinner_msg);
                }
            }
        }
    }

    let status = child.wait().await;
    let err_output = err_handle.await.unwrap_or_default();
    let is_success = status.as_ref().is_ok_and(|s| s.success());
    let exit_code = status.as_ref().map_or(1, |s| s.code().unwrap_or(1));

    if !is_success {
        spinner.finish_and_clear();
        println!(
            "{} operation aborted or failed (code {}):\n{}",
            "✗".red(),
            exit_code,
            err_output.trim().red()
        );
        std::process::exit(exit_code);
    }

    if !hook_alerts.is_empty() {
        spinner.finish_and_clear();
        println!("{} {}", "✓".green(), success_msg);
        println!(
            "\n{}",
            "!!! changes completed, but warnings/errors occurred during hooks:"
                .yellow()
                .bold()
        );
        for alert in hook_alerts {
            println!("  {}", alert.yellow());
        }
        println!();
        return;
    }

    spinner.finish_and_clear();
    println!("{} {}", "✓".green(), success_msg);
}

async fn process_installation(packages: Vec<String>, alpm_handle: alpm::Alpm, cli: &Cli) {
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
        match core::resolver::get_install_summaries(&alpm_handle, &native_pkgs) {
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
            println!("{} failed to query the aur.", "✗".red());
        }
    }

    drop(alpm_handle);

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
        println!("\n{:<15} {:.2} MB", "download:", total_dl);
        println!("{:<15} {:.2} MB", "disk usage:", total_inst);
    } else {
        println!();
    }

    if !cli.noconfirm {
        let has_aur = !resolved_aur_pkgs.is_empty();
        let mut prompt_msg = if has_aur {
            "Proceed with installation? [Y/n/v] (v = view PKGBUILDs)".to_string()
        } else {
            "Proceed with installation? [Y/n]".to_string()
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
        args.extend(native_pkgs.iter().map(|s| s.as_str()));

        run_pacman(
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

                    let pacman_args = vec!["-U", pkg_path.to_str().unwrap(), "--noconfirm"];

                    run_pacman(
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
        .expect("failed to open lock file");

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
            run_pacman(
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
            let alpm_handle = core::alpm_init::init_alpm()?;
            let local_db = alpm_handle.localdb();

            match cmd {
                Commands::Tui | Commands::Completions { .. } | Commands::Update => unreachable!(),

                Commands::Install { packages } => {
                    process_installation(packages, alpm_handle, &cli).await;
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
                        let check_spinner = ui::progress::spinner("querying aur...");

                        if let Ok(response) = reqwest::get(&aur_url).await
                            && let Ok(json) = response.json::<serde_json::Value>().await
                            && let Some(aur_results) =
                                json.get("results").and_then(|r| r.as_array())
                        {
                            check_spinner.finish_and_clear();
                            for pkg in aur_results {
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
                                    name.to_string(),
                                    version.to_string(),
                                    status,
                                    desc.to_string(),
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
                    std::io::stdin().read_line(&mut input).unwrap();

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

                    process_installation(pkgs_to_install, alpm_handle, &cli).await;
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
                        if !prompt_confirm("proceed with removal? [Y/n]") {
                            println!("{} aborted.", "✗".red());
                            return Ok(());
                        }
                    }

                    drop(alpm_handle);

                    let mut args = vec!["-Rs", "--noconfirm"];
                    args.extend(packages.iter().map(|s| s.as_str()));

                    run_pacman(
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
                            .status()
                            .expect("failed to sync databases");

                        if !status.success() {
                            println!("{} failed to sync databases.", "✗".red());
                            return Ok(());
                        }
                    }

                    let mut aur_updates = Vec::new();
                    if !foreign_pkgs.is_empty() && !cli.repo {
                        let check_spinner = ui::progress::spinner("querying aur for updates...");

                        for chunk in foreign_pkgs.chunks(50) {
                            let mut url = String::from("https://aur.archlinux.org/rpc/v5/info?");
                            for (name, _) in chunk {
                                url.push_str(&format!("arg[]={}&", name));
                            }

                            if let Ok(response) = reqwest::get(&url).await
                                && let Ok(json) = response.json::<serde_json::Value>().await
                                && let Some(results) =
                                    json.get("results").and_then(|r| r.as_array())
                            {
                                for result in results {
                                    if let Some(name) = result.get("Name").and_then(|n| n.as_str())
                                        && let Some(new_ver) =
                                            result.get("Version").and_then(|v| v.as_str())
                                        && let Some((_, local_ver)) =
                                            chunk.iter().find(|(n, _)| n == name)
                                        && alpm::vercmp(local_ver.as_str(), new_ver)
                                            == std::cmp::Ordering::Less
                                    {
                                        aur_updates.push((
                                            name.to_string(),
                                            local_ver.clone(),
                                            new_ver.to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                        check_spinner.finish_and_clear();
                    }

                    let mut native_lines: Vec<String> = Vec::new();
                    if !cli.aur {
                        let qu_output = std::process::Command::new("pacman")
                            .arg("-Qu")
                            .output()
                            .expect("failed to query updates");

                        let updates = String::from_utf8_lossy(&qu_output.stdout);
                        native_lines = updates
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .map(|s| s.to_string())
                            .collect();
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
                    display_arch_news().await;

                    if !cli.noconfirm && !prompt_confirm("proceed with upgrade? [Y/n]") {
                        println!("{} aborted.", "✗".red());
                        return Ok(());
                    }

                    if !native_lines.is_empty() {
                        run_pacman(
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

                                    run_pacman(
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

                        run_pacman(
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
                            && let Ok(json) = response.json::<serde_json::Value>().await
                            && let Some(results) = json.get("results").and_then(|r| r.as_array())
                            && !results.is_empty()
                        {
                            found = true;
                            print_header();

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
                            format!("{:.2} MB", bytes as f64 / 1_048_576.0)
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
                        println!("\n{:<15} {:.2} MB", "wasted space:", total_size);
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

                    run_pacman(
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

                    run_pacman(
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

                    run_pacman(
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
                    core::pacnew::manage_pacnew_files();
                }

                Commands::Pkgbuild { package } => {
                    drop(alpm_handle);
                    view_pkgbuilds(&[package]).await;
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
                    let spinner = ui::progress::spinner("scanning system metrics...");

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
                            format!("{:.2} GiB", bytes / 1_073_741_824.0)
                        } else {
                            format!("{:.2} MiB", bytes / 1_048_576.0)
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
                        .unwrap_or_else(|| "Arch Linux".to_string());

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
                        if !prompt_confirm(&format!(
                            "Install all packages in group '{}'? [Y/n]",
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

                    run_pacman(
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
