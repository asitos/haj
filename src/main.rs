mod cli;
mod config;
mod core;
mod network;
mod tui;
mod ui;

use clap::Parser;
use cli::{Cli, Commands};
use fs2::FileExt;
use owo_colors::OwoColorize;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

// helper function for Y/n prompt
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
        let root_arg = root.as_ref().map_or(String::new(), |r| format!("--root {} ", r));
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
                    last_spinner_msg = format!("  {}", "downloading packages...".dimmed());
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
                        last_spinner_msg = format!("{}    {}", ":3".yellow(), clean.dimmed());
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
                            println!("  {}", line.dimmed());
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
        spinner.finish_with_message(format!(
            "{} operation aborted or failed (code {}):\n{}",
            "✗".red(),
            exit_code,
            err_output.trim().red()
        ));
        std::process::exit(exit_code);
    }

    if !hook_alerts.is_empty() {
        spinner.finish_with_message(format!("{} {}", "✓".green(), success_msg));
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

    spinner.finish_with_message(format!("{} {}", "✓".green(), success_msg));
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o666)
        .open("/tmp/haj.lock")
        .expect("failed to open lock file");

    if lock_file.try_lock_exclusive().is_err() {
        println!(
            "{} haj is currently running in another terminal. (waiting for lock...)",
            "✗".red()
        );
        lock_file.lock_exclusive().expect("failed to acquire lock");
    }

    let _haj_lock = lock_file;

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
                cli.verbose,
                &cli.root,
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
                                if let Some(result) = results.iter().find(|r| r.get("Name").and_then(|n| n.as_str()) == Some(&pkg)) {
                                    if let Some(aur_ver) = result.get("Version").and_then(|v| v.as_str()) {
                                        let mut is_update = false;
                                        let mut skip = false;

                                        if let Some(lv) = &local_ver {
                                            if lv == aur_ver {
                                                println!("{} {} is up to date ({}).", "✓".green(), pkg.magenta().bold(), aur_ver.dimmed());
                                                skip = true;
                                            } else {
                                                is_update = true;
                                            }
                                        }

                                        if !skip {
                                            resolved_aur_pkgs.push((pkg.clone(), aur_ver.to_string(), is_update, local_ver.clone()));
                                        }
                                    }
                                } else {
                                    println!("{} package '{}' not found on the aur.", "✗".red(), pkg.bold());
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
                        return Ok(());
                    }

                    println!("\n{}", "targets:".bold().white());

                    if !native_summaries.is_empty() {
                        println!("  {}", "native repositories:".dimmed());
                        for sum in &native_summaries {
                            println!("    {:<25} {}", sum.name.cyan().bold(), sum.version.dimmed());
                        }
                    }

                    if !resolved_aur_pkgs.is_empty() {
                        println!("  {}", "arch user repository:".dimmed());
                        for (pkg, aur_ver, is_update, local_ver) in &resolved_aur_pkgs {
                            if *is_update {
                                println!(
                                    "    {:<25} {} -> {}",
                                    pkg.magenta().bold(),
                                    local_ver.as_ref().unwrap().red(),
                                    aur_ver.green()
                                );
                            } else {
                                println!("    {:<25} {}", pkg.magenta().bold(), aur_ver.green());
                            }
                        }
                    }

                    if total_dl > 0.0 || total_inst > 0.0 {
                        println!("\n{:<15} {:.2} MB", "download:", total_dl);
                        println!("{:<15} {:.2} MB", "disk usage:", total_inst);
                    } else {
                        println!(); 
                    }

                    if !cli.noconfirm && !prompt_confirm("Proceed with installation? [Y/n]") {
                        println!("{} aborted.", "✗".red());
                        return Ok(());
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
                                pkg.magenta().bold(),
                                aur_ver.green()
                            );

                            match core::aur::build(&pkg, cli.verbose).await {
                                Ok(pkg_path) => {
                                    let spinner_msg = if is_update {
                                        format!("updating built package {}...", pkg.magenta().bold())
                                    } else {
                                        format!("installing built package {}...", pkg.magenta().bold())
                                    };

                                    let success_msg = if is_update {
                                        format!(
                                            "{} updated successfully ({}).",
                                            pkg.magenta().bold(),
                                            aur_ver.dimmed()
                                        )
                                    } else {
                                        format!(
                                            "{} installed successfully ({}).",
                                            pkg.magenta().bold(),
                                            aur_ver.dimmed()
                                        )
                                    };

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

                    let print_cmd = std::process::Command::new("pacman")
                        .arg("-Rsp")
                        .args(packages)
                        .output()
                        .expect("failed to execute pacman");

                    if !print_cmd.status.success() {
                        println!(
                            "{} failed to resolve dependencies. (do these packages conflict?)",
                            "✗".red()
                        );
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
                        if !prompt_confirm("Proceed with removal? [Y/n]") {
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

                Commands::Upgrade { sysupgrade } => {
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

                    if *sysupgrade {
                        println!("{} syncing package databases...\n", "::".blue().bold());
                        let status = std::process::Command::new("sudo")
                            .args(["pacman", "-Sy"])
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
                            println!("  {}", line.cyan());
                        }
                    }

                    for (name, old, new) in &aur_updates {
                        println!(
                            "  {:<30} {} -> {}",
                            name.magenta().bold(),
                            old.red(),
                            new.green()
                        );
                    }

                    let total_upgrades = native_lines.len() + aur_updates.len();
                    println!("\n{:<15} {}", "total:", total_upgrades.to_string().cyan());

                    if !cli.noconfirm && !prompt_confirm("Proceed with upgrade? [Y/n]") {
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
                                name.magenta().bold(),
                                new_ver.green()
                            );

                            match core::aur::build(&name, cli.verbose).await {
                                Ok(pkg_path) => {
                                    let spinner_msg = format!(
                                        "updating built package {}...",
                                        name.magenta().bold()
                                    );
                                    let success_msg = format!(
                                        "{} updated successfully ({}).",
                                        name.magenta().bold(),
                                        new_ver.dimmed()
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
                    core::history::show_history(*limit);
                }

                Commands::Downgrade { package } => {
                    drop(alpm_handle);

                    if let Some(archive_path) = core::downgrade::select_downgrade_target(package) {
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
                    core::cache::scrub(*keep);
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
                        cli.verbose,
                        &cli.root,
                    )
                    .await;
                }

                Commands::Diff => {
                    drop(alpm_handle);
                    core::pacnew::manage_pacnew_files();
                }

                Commands::List {
                    explicit,
                    deps,
                    foreign,
                } => {
                    let mut count = 0;

                    for pkg in local_db.pkgs() {
                        let is_explicit = pkg.reason() == alpm::PackageReason::Explicit;

                        if *explicit && !is_explicit {
                            continue;
                        }
                        if *deps && is_explicit {
                            continue;
                        }

                        let mut found_in_repo = false;
                        for db in alpm_handle.syncdbs() {
                            if db.pkg(pkg.name()).is_ok() {
                                found_in_repo = true;
                                break;
                            }
                        }

                        if *foreign && found_in_repo {
                            continue;
                        }

                        if found_in_repo {
                            println!("{} {}", pkg.name().cyan(), pkg.version().dimmed());
                        } else {
                            println!("{} {}", pkg.name().magenta(), pkg.version().dimmed());
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
                    println!("{} scanning system metrics...\n", "::".blue());

                    let mut native_count = 0;
                    let mut foreign_count = 0;
                    // let mut explicit_count = 0;
                    let mut total_size_bytes: i64 = 0;
                    let mut orphan_count = 0;
                    let mut orphan_size_bytes: i64 = 0;

                    for pkg in local_db.pkgs() {
                        let isize = pkg.isize();
                        total_size_bytes += isize;

                        let is_explicit = pkg.reason() == alpm::PackageReason::Explicit;
                        // if is_explicit {
                        // explicit_count += 1;
                        // }

                        if !is_explicit
                            && pkg.required_by().is_empty()
                            && pkg.optional_for().is_empty()
                        {
                            orphan_count += 1;
                            orphan_size_bytes += isize;
                        }

                        let mut found_in_repo = false;
                        for db in alpm_handle.syncdbs() {
                            if db.pkg(pkg.name()).is_ok() {
                                found_in_repo = true;
                                break;
                            }
                        }

                        if found_in_repo {
                            native_count += 1;
                        } else {
                            foreign_count += 1;
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

                    let format_gb = |bytes: f64| -> String {
                        if bytes > 1_073_741_824.0 {
                            format!("{:.2} GB", bytes / 1_073_741_824.0)
                        } else {
                            format!("{:.2} MB", bytes / 1_048_576.0)
                        }
                    };

                    println!("{}", "✓ system health & statistics".bold().white());

                    println!("\n  {} packages", ":3".cyan());
                    println!(
                        "     {:<12} {}",
                        "total:",
                        (native_count + foreign_count).to_string().white().bold()
                    );
                    println!("     {:<12} {}", "native:", native_count.to_string().cyan());
                    println!(
                        "     {:<12} {}",
                        "aur:",
                        foreign_count.to_string().magenta()
                    );

                    println!("\n  {} disk usage", ":O".blue());
                    println!(
                        "     {:<12} {}",
                        "installed:",
                        format_gb(total_size_bytes as f64).green()
                    );
                    if orphan_count > 0 {
                        println!(
                            "     {:<12} {} ({})",
                            "orphans:",
                            orphan_count.to_string().yellow(),
                            format_gb(orphan_size_bytes as f64).dimmed()
                        );
                    } else {
                        println!("     {:<12} {}", "orphans:", "0 (clean)".green());
                    }
                    println!(
                        "     {:<12} {} (pacman) / {} (aur)",
                        "cache:",
                        format_gb(pacman_cache as f64).dimmed(),
                        format_gb(aur_cache as f64).dimmed()
                    );

                    println!("\n  {} databases", ":p".magenta());
                    println!("     {:<12} {}", "last sync:", sync_time.cyan());
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
                        name.cyan().bold()
                    );
                    for (pkg_name, pkg_ver, is_installed) in &group_pkgs {
                        let status = if *is_installed {
                            format!(" {}", "[installed]".cyan().bold())
                        } else {
                            "".to_string()
                        };
                        println!("  {} {}{}", pkg_name.bold(), pkg_ver.dimmed(), status);
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
                        &format!("installing group {}...", name.cyan()),
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
