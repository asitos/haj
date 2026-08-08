use crate::cli::Cli;
use crate::core::ui::prompt_confirm;
use crate::ui;
use crossterm::style::Stylize;

pub async fn run_pacman(
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
            .map_or(String::new(), |r| format!("--root {r} "));
        println!("{arrow} would execute: sudo pacman {root_arg}{cmd}");
        return;
    }

    if let Err(e) = crate::core::ensure_sudo().await {
        println!("{} {}", "✗".red(), e);
        return;
    }

    let is_root = crate::core::is_root();

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
        let mut child = match child_cmd
            .args(args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                print_network_error(&format!("{} failed to spawn pacman: {}", "✗".red(), e));
                return;
            }
        };

        let status = child.wait().await;
        if status.is_ok_and(|s| s.success()) {
            println!("{} {}", "✓".green(), success_msg);
        } else {
            print_network_error(&format!("{} operation failed.", "✗".red()));
        }
        return;
    }

    child_cmd.arg("--color=never");

    let mut spinner = ui::spinner(spinner_msg);
    let mut last_spinner_msg = spinner_msg.to_string();
    let mut context_buffer: Vec<String> = Vec::new();
    let mut in_hook_phase = false;

    let mut child = match child_cmd
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            spinner.finish_and_clear();
            print_network_error(&format!("{} failed to spawn pacman: {}", "✗".red(), e));
            return;
        }
    };

    let mut stdin = if let Some(s) = child.stdin.take() {
        s
    } else {
        return;
    };
    let mut stdout = if let Some(s) = child.stdout.take() {
        s
    } else {
        return;
    };
    let mut stderr = if let Some(s) = child.stderr.take() {
        s
    } else {
        return;
    };

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
                } else if clean.contains("retrieving packages") || clean.contains("downloading") {
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
                } else if clean.contains("running pre-transaction hooks")
                    || clean.contains("running post-transaction hooks")
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
                                            print!("{c}");
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
                    spinner = ui::spinner(&last_spinner_msg);
                }
            }
        }
    }

    let status = child.wait().await;
    let err_output = err_handle.await.unwrap_or_default();
    let is_success = status.as_ref().is_ok_and(std::process::ExitStatus::success);
    let exit_code = status.as_ref().map_or(1, |s| s.code().unwrap_or(1));

    if !is_success {
        spinner.finish_and_clear();
        let fallback = format!(
            "{} operation aborted or failed (code {}):\n{}",
            "✗".red(),
            exit_code,
            err_output.trim().red()
        );
        print_network_error(&fallback);
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

pub async fn check_and_offer_sync(cli: &Cli) {
    if cli.dry_run {
        return;
    }

    let mut needs_sync = false;
    let mut missing = true;
    let sync_dir =
        std::path::Path::new(cli.root.as_deref().unwrap_or("/")).join("var/lib/pacman/sync");

    if let Ok(entries) = std::fs::read_dir(&sync_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("db") {
                missing = false;
                if let Ok(metadata) = entry.metadata()
                    && let Ok(modified) = metadata.modified()
                    && let Ok(duration) = std::time::SystemTime::now().duration_since(modified)
                    && duration.as_secs() > 7 * 24 * 60 * 60
                {
                    needs_sync = true;
                    break;
                }
            }
        }
    }

    if missing || needs_sync {
        let msg = if missing {
            "package databases are missing."
        } else {
            "package databases are stale (older than 7 days)."
        };
        println!("\n{} {}", "::".blue(), msg);
        if cli.noconfirm || prompt_confirm("run 'haj sync' now? [Y/n]") {
            run_pacman(
                &["-Sy"],
                "syncing package databases from mirrors...",
                "repositories synced successfully.",
                cli.dry_run,
                cli.verbose,
                &cli.root,
            )
            .await;
        }
    }
}

pub fn network_error_message(fallback_msg: &str, is_connected: bool) -> String {
    if is_connected {
        fallback_msg.to_string()
    } else {
        format!(
            "{} haj cannot surf the internet, check your internet connection.",
            "✗".red()
        )
    }
}

pub fn print_network_error(fallback_msg: &str) {
    let ping_status = std::process::Command::new("ping")
        .arg("-c")
        .arg("1")
        .arg("-W")
        .arg("2")
        .arg("archlinux.org")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    let is_connected = ping_status.is_ok_and(|s| s.success());
    println!("{}", network_error_message(fallback_msg, is_connected));
}
