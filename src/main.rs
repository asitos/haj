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
) {
    if is_dry_run {
        println!(
            "{} no system changes will be made.",
            "[dry run]".bold().yellow()
        );
        let arrow = "→".cyan();
        let cmd = args.join(" ");
        println!("{arrow} would execute: sudo pacman {cmd}");
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
    
    let is_noconfirm = args.iter().any(|&a| a == "--noconfirm");

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

    if !is_success {
        spinner.finish_with_message(format!(
            "{} operation aborted or failed (code {}):\n{}",
            "✗".red(),
            status.as_ref().map_or(1, |s| s.code().unwrap_or(1)),
            err_output.trim().red()
        ));
        return;
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
                    let mut do_native_install = false;

                    if !native_pkgs.is_empty() {
                        println!("{} resolving native dependencies...\n", "✓".green());

                        match core::resolver::get_install_summaries(&alpm_handle, &native_pkgs) {
                            Ok(summaries) => {
                                let mut total_dl = 0.0;
                                let mut total_inst = 0.0;

                                println!("{}", "installing (native)".bold().white());
                                for sum in &summaries {
                                    println!(
                                        "  {} {}",
                                        sum.name.cyan().bold(),
                                        sum.version.dimmed()
                                    );
                                    total_dl += sum.download_size_mb;
                                    total_inst += sum.install_size_mb;
                                }

                                println!("\n{:<15} {:.2} MB", "download:", total_dl);
                                println!("{:<15} {:.2} MB", "disk usage:", total_inst);

                                if !cli.noconfirm {
                                    println!();
                                    if !prompt_confirm("Continue with native packages? [Y/n]") {
                                        do_native_install = false;
                                        println!("{} skipped native packages.", "✗".red());
                                    } else {
                                        do_native_install = true;
                                    }
                                } else {
                                    do_native_install = true;
                                }
                            }
                            Err(e) => println!("{} {}", "✗".red(), e),
                        }
                    }

                    // release var/lib/pacman/db.lck
                    drop(alpm_handle);

                    if do_native_install {
                        let mut args = vec!["-S", "--noconfirm"];
                        args.extend(native_pkgs.iter().map(|s| s.as_str()));

                        run_pacman(
                            &args,
                            "installing native packages...",
                            "native packages installed successfully.",
                            cli.dry_run,
                            cli.verbose,
                        )
                        .await;
                    }

                    for (pkg, local_ver) in aur_pkgs {
                        if cli.dry_run {
                            println!(
                                "{} would build and install aur package: {}",
                                "[dry run]".bold().yellow(),
                                pkg.magenta()
                            );
                            continue;
                        }

                        let check_spinner = ui::progress::spinner(&format!(
                            "{} querying aur for {}...",
                            "::".blue(),
                            pkg.magenta().bold()
                        ));
                        let aur_url =
                            format!("https://aur.archlinux.org/rpc/v5/info?arg[]={}", pkg);
                        let mut aur_ver = String::new();

                        if let Ok(response) = reqwest::get(&aur_url).await
                            && let Ok(json) = response.json::<serde_json::Value>().await
                            && let Some(results) = json.get("results").and_then(|r| r.as_array())
                            && let Some(first) = results.first()
                            && let Some(v) = first.get("Version").and_then(|v| v.as_str())
                        {
                            aur_ver = v.to_string();
                        }

                        check_spinner.finish_and_clear();

                        if aur_ver.is_empty() {
                            println!(
                                "{} package '{}' not found on the aur.",
                                "✗".red(),
                                pkg.bold()
                            );
                            continue;
                        }

                        let mut is_update = false;
                        if let Some(lv) = &local_ver {
                            if lv == &aur_ver {
                                println!(
                                    "{} {} is up to date ({}). nothing to do.",
                                    "✓".green(),
                                    pkg.magenta().bold(),
                                    aur_ver.dimmed()
                                );
                                continue;
                            }
                            is_update = true;
                            println!(
                                "\n{} preparing to update {} ({} -> {})...",
                                "::".blue(),
                                pkg.magenta().bold(),
                                lv.red(),
                                aur_ver.green()
                            );
                        } else {
                            println!(
                                "\n{} preparing to install {} ({})...",
                                "::".blue(),
                                pkg.magenta().bold(),
                                aur_ver.green()
                            );
                        }

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
                                )
                                .await;
                            }
                            Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
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
                    )
                    .await;
                }

                Commands::Upgrade { sysupgrade } => {
                    // lock release baby
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

                    let qu_output = std::process::Command::new("pacman")
                        .arg("-Qu")
                        .output()
                        .expect("failed to query updates");

                    let updates = String::from_utf8_lossy(&qu_output.stdout);
                    let lines: Vec<&str> =
                        updates.lines().filter(|l| !l.trim().is_empty()).collect();

                    if lines.is_empty() {
                        println!("{} system is fully up to date!", "✓".green());
                        return Ok(());
                    }

                    println!("{}", "available upgrades:".bold().white());
                    for line in &lines {
                        // lines look like: "core/linux 6.9.1-1 -> 6.9.2-1"
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

                    println!("\n{:<15} {}", "total:", lines.len().to_string().cyan());

                    if !cli.noconfirm && !prompt_confirm("Proceed with upgrade? [Y/n]") {
                        println!("{} aborted.", "✗".red());
                        return Ok(());
                    }

                    run_pacman(
                        &["-Su", "--noconfirm"],
                        "upgrading system packages...",
                        "system upgraded successfully.",
                        cli.dry_run,
                        cli.verbose,
                    )
                    .await;
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
