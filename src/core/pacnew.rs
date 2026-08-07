use crossterm::style::Stylize;
use std::process::{Command, Stdio};

pub fn manage_pacnew_files() -> anyhow::Result<()> {
    println!("{} launching pacdiff...", "::".blue());

    let mut child = Command::new("sudo")
        .arg("pacdiff")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to launch pacdiff. is pacman-contrib installed? ({})",
                e
            )
        })?;

    let status = child
        .wait()
        .map_err(|e| anyhow::anyhow!("failed to wait on pacdiff: {}", e))?;

    if status.success() {
        println!("{} pacnew management complete.", "✓".green());
    } else {
        println!("{} pacnew management aborted or failed.", "✗".red());
    }

    Ok(())
}
