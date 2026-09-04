#![allow(dead_code, clippy::collapsible_if)]
use crate::backend::traits::{BackendError, CommandPlan};
use crate::ui;
use crossterm::style::Stylize;
use tokio::io::AsyncReadExt;

pub async fn execute_plan(
    plan: &CommandPlan,
    spinner_msg: &str,
    success_msg: &str,
    is_dry_run: bool,
    is_verbose: bool,
) -> Result<(), BackendError> {
    if is_dry_run {
        println!(
            "{} no system changes will be made.",
            "[dry run]".bold().yellow()
        );
        let arrow = "→".cyan();
        let cmd = format!("{} {}", plan.executable, plan.args.join(" "));
        let prefix = if plan.requires_root { "sudo " } else { "" };
        println!("{arrow} would execute: {prefix}{cmd}");
        return Ok(());
    }

    let is_root = crate::core::is_root();

    if plan.requires_root && !is_root {
        if let Err(e) = crate::core::ensure_sudo().await {
            println!("{} {}", "✗".red(), e);
            return Err(BackendError::ExecutionError(
                1,
                "sudo authentication failed".to_string(),
            ));
        }
    }

    let mut child_cmd = if plan.requires_root && !is_root {
        let mut c = tokio::process::Command::new("sudo");
        c.arg(&plan.executable);
        c
    } else {
        tokio::process::Command::new(&plan.executable)
    };

    if is_verbose {
        println!(
            "{} [verbose] executing: {} {}",
            "::".blue(),
            plan.executable,
            plan.args.join(" ")
        );
        let mut child = match child_cmd
            .args(&plan.args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let err = format!("failed to spawn command: {}", e);
                println!("{} {}", "✗".red(), err);
                return Err(BackendError::ExecutionError(1, err));
            }
        };

        let status = child
            .wait()
            .await
            .unwrap_or_else(|_| std::process::ExitStatus::default());
        if status.success() {
            println!("{} {}", "✓".green(), success_msg);
            return Ok(());
        } else {
            println!("{} operation failed.", "✗".red());
            return Err(BackendError::ExecutionError(
                status.code().unwrap_or(1),
                "operation failed".to_string(),
            ));
        }
    }

    child_cmd.arg("--color=never");

    let spinner = ui::spinner(spinner_msg);

    let mut child = match child_cmd
        .args(&plan.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            spinner.finish_and_clear();
            let err = format!("failed to spawn command: {}", e);
            println!("{} {}", "✗".red(), err);
            return Err(BackendError::ExecutionError(1, err));
        }
    };

    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let err_handle = tokio::spawn(async move {
        let mut err_str = String::new();
        let mut buf = [0u8; 1024];
        while let Ok(n) = stderr.read(&mut buf).await {
            if n == 0 {
                break;
            }
            err_str.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
        err_str
    });

    let mut buf = [0u8; 128];
    let mut current_line = String::new();

    while let Ok(n) = stdout.read(&mut buf).await {
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

                // Extremely simple spinner update based on current line.
                // Pacman's complex hook parsing remains in run_pacman if we don't fully migrate it,
                // but a generic executor should just show the latest output.
                if clean.len() > 3 {
                    let last_status: String = clean.chars().take(60).collect();
                    spinner.set_message(format!("{}...", last_status));
                }
                current_line.clear();
            } else {
                current_line.push(c);
            }
        }
    }

    spinner.finish_and_clear();
    let status = child
        .wait()
        .await
        .unwrap_or_else(|_| std::process::ExitStatus::default());
    let err_output = err_handle.await.unwrap_or_default();

    if status.success() {
        println!("{} {}", "✓".green(), success_msg);
        Ok(())
    } else {
        println!("{} operation failed.\n{}", "✗".red(), err_output.trim());
        Err(BackendError::ExecutionError(
            status.code().unwrap_or(1),
            err_output,
        ))
    }
}
