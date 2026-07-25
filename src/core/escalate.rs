use tokio::process::Command;
use owo_colors::OwoColorize;

pub async fn ensure_sudo() -> anyhow::Result<()> {
    // roooot check
    let is_root = unsafe { libc::geteuid() == 0 };
    if is_root {
        return Ok(());
    }

    let check = Command::new("sudo")
        .args(["-n", "-v"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await?;

    if !check.success() {
        println!("{} haj demands elevated privileges.", "::".blue().bold());
        let auth = Command::new("sudo")
            .arg("-v")
            .status()
            .await?;

        if !auth.success() {
            anyhow::bail!("authentication failed or aborted.");
        }
    }

    Ok(())
}
