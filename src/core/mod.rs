#[cfg(feature = "pacman")]
pub mod alpm_init;
#[cfg(feature = "pacman")]
pub mod aur;
pub mod cache;
#[cfg(feature = "pacman")]
pub mod conflicts;
#[cfg(feature = "pacman")]
pub mod downgrade;
pub mod history;
pub mod package;
#[cfg(feature = "pacman")]
pub mod pacman;
pub mod process;
pub mod ui;

#[cfg(feature = "pacman")]
use alpm::Alpm;
#[allow(unused_imports)]
use anyhow::Result;
use crossterm::style::Stylize;
use std::process::Stdio;
use tokio::process::Command;

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub async fn ensure_sudo() -> anyhow::Result<()> {
    if is_root() {
        return Ok(());
    }

    let check = Command::new("sudo")
        .args(["-n", "-v"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;

    if !check.success() {
        println!("{} haj demands elevated privileges.", "::".blue().bold());
        let auth = Command::new("sudo").arg("-v").status().await?;

        if !auth.success() {
            anyhow::bail!("authentication failed or aborted.");
        }
    }

    Ok(())
}

#[cfg(feature = "pacman")]
#[allow(dead_code)]
pub fn manage_pacnew_files() -> anyhow::Result<()> {
    println!("{} launching pacdiff...", "::".blue());

    let mut child = std::process::Command::new("sudo")
        .arg("pacdiff")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!("failed to launch pacdiff. is pacman-contrib installed? ({e})")
        })?;

    let status = child
        .wait()
        .map_err(|e| anyhow::anyhow!("failed to wait on pacdiff: {e}"))?;

    if status.success() {
        println!("{} pacnew management complete.", "✓".green());
    } else {
        println!("{} pacnew management aborted or failed.", "✗".red());
    }

    Ok(())
}

#[cfg(feature = "pacman")]
pub struct InstallSummary {
    pub name: String,
    pub version: String,
    pub download_size_mb: f64,
    pub install_size_mb: f64,
}

#[cfg(feature = "pacman")]
pub fn get_install_summaries(alpm: &Alpm, targets: &[String]) -> Result<Vec<InstallSummary>> {
    let mut summaries = Vec::new();

    for target in targets {
        let mut found = false;

        for db in alpm.syncdbs() {
            if let Ok(pkg) = db.pkg(target.as_str()) {
                summaries.push(InstallSummary {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    download_size_mb: pkg.download_size() as f64 / 1024.0 / 1024.0,
                    install_size_mb: pkg.isize() as f64 / 1024.0 / 1024.0,
                });
                found = true;
                break;
            }
        }

        if !found {
            anyhow::bail!("package '{target}' not found in any repository.");
        }
    }

    Ok(summaries)
}
