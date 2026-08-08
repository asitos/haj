use crossterm::style::Stylize;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AurPackage {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub num_votes: u64,
}

#[derive(Debug, Deserialize)]
pub struct AurResponse {
    pub results: Vec<AurPackage>,
}

pub async fn build(pkg_name: &str, is_verbose: bool) -> anyhow::Result<PathBuf> {
    // roooooot user check ahhhhh
    if crate::core::is_root() {
        anyhow::bail!(
            "{} makepkg cannot run as root. please run haj as a normal user.",
            "✗".red().bold()
        );
    }

    crate::core::ensure_sudo().await?;

    let home = std::env::var("HOME")
        .map_err(|e| anyhow::anyhow!("HOME environment variable not set: {}", e))?;
    let aur_cache_dir = PathBuf::from(home).join(".cache/haj/aur");
    tokio::fs::create_dir_all(&aur_cache_dir).await?;

    let pkg_dir = aur_cache_dir.join(pkg_name);

    let fetch_spinner =
        crate::ui::spinner(&format!("fetching {} from aur...", pkg_name.bold()));

    if pkg_dir.exists() {
        let status = Command::new("git")
            .arg("-C")
            .arg(&pkg_dir)
            .arg("pull")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;

        if !status.success() {
            fetch_spinner.finish_and_clear();
            anyhow::bail!("failed to pull latest changes for {}", pkg_name);
        }
    } else {
        let aur_url = format!("https://aur.archlinux.org/{}.git", pkg_name);
        let status = Command::new("git")
            .arg("clone")
            .arg(&aur_url)
            .arg(&pkg_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;

        if !status.success() {
            fetch_spinner.finish_and_clear();
            anyhow::bail!("failed to clone {}. does it exist?", pkg_name);
        }
    }
    fetch_spinner.finish_and_clear();

    let dep_spinner = crate::ui::spinner("resolving dependencies...");

    let srcinfo_out = Command::new("makepkg")
        .arg("--printsrcinfo")
        .current_dir(&pkg_dir)
        .output()
        .await?;

    if !srcinfo_out.status.success() {
        dep_spinner.finish_and_clear();
        anyhow::bail!("failed to read PKGBUILD for {}", pkg_name);
    }

    let srcinfo_str = String::from_utf8_lossy(&srcinfo_out.stdout);
    let mut all_deps = Vec::new();
    for line in srcinfo_str.lines() {
        let line = line.trim();
        if (line.starts_with("depends =") || line.starts_with("makedepends ="))
            && let Some(dep) = line.split(" = ").nth(1)
        {
            all_deps.push(dep.to_string());
        }
    }

    let mut missing_deps = Vec::new();
    if !all_deps.is_empty() {
        let pacman_t = Command::new("pacman")
            .arg("-T")
            .args(&all_deps)
            .output()
            .await?;

        let missing_str = String::from_utf8_lossy(&pacman_t.stdout);
        for dep in missing_str.lines() {
            let dep = dep.trim();
            if !dep.is_empty() {
                missing_deps.push(dep.to_string());
            }
        }
    }
    dep_spinner.finish_and_clear();

    if !missing_deps.is_empty() {
        let install_spinner = crate::ui::spinner(&format!(
            "installing {} missing dependencies...",
            missing_deps.len()
        ));

        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-S")
            .arg("--noconfirm")
            .args(&missing_deps)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;

        install_spinner.finish_and_clear();

        if !status.success() {
            anyhow::bail!(
                "failed to install missing build dependencies. (one of them might be an aur package). run 'pacman -S {}' manually to debug.",
                missing_deps.join(" ")
            );
        }
    }

    if is_verbose {
        println!(
            "{} [verbose] building {} with native output...",
            "::".blue(),
            pkg_name.bold()
        );
        let mut child = Command::new("makepkg")
            .current_dir(&pkg_dir)
            .args(["-cf", "--noconfirm", "--nocheck"])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        let status = child.wait().await?;
        if !status.success() {
            anyhow::bail!("makepkg failed with code {}", status.code().unwrap_or(1));
        }
    } else {
        let build_spinner =
            crate::ui::spinner(&format!("preparing to build {}...", pkg_name.bold()));

        let mut child = Command::new("makepkg")
            .current_dir(&pkg_dir)
            .args(["-cf", "--noconfirm", "--nocheck"])
            .stdin(Stdio::null()) // keyboard disconnect
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture stderr"))?;

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
                    if clean.starts_with("==> Making package:") {
                        build_spinner.set_message(format!(
                            "{} initializing build environment...",
                            "::".blue()
                        ));
                    } else if clean.starts_with("==> Retrieving sources") {
                        build_spinner
                            .set_message(format!("{} downloading source code...", "::".blue()));
                    } else if clean.starts_with("==> Starting build()") {
                        build_spinner.set_message(format!(
                            "{} compiling {} (this may take a while)...",
                            "::".blue(),
                            pkg_name.bold()
                        ));
                    } else if clean.starts_with("==> Starting package()") {
                        build_spinner.set_message(format!("{} packaging binary...", "::".blue()));
                    }
                    current_line.clear();
                } else {
                    current_line.push(c);
                }
            }
        }

        let status = child.wait().await?;
        build_spinner.finish_and_clear();

        let err_output = err_handle.await.unwrap_or_default();

        if !status.success() {
            anyhow::bail!(
                "makepkg failed (code {}):\n{}",
                status.code().unwrap_or(1),
                err_output.trim().red()
            );
        }
    }

    let mut built_pkg = None;
    let mut entries = tokio::fs::read_dir(&pkg_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            if file_name.contains(".pkg.tar.") && !file_name.contains("-debug-") {
                built_pkg = Some(path);
                break;
            }
        }
    }

    match built_pkg {
        Some(path) => Ok(path),
        None => anyhow::bail!("build succeeded but could not locate the .pkg.tar archive"),
    }
}
