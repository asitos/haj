use crossterm::style::Stylize;
use std::process::Command;

pub fn scrub(keep: usize) {
    println!(
        "{} scrubbing package cache (keeping last {} versions)...",
        "::".blue(),
        keep
    );

    let keep_str = keep.to_string();

    let native_status = Command::new("paccache")
        .args(["-r", "-k", &keep_str])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    if native_status.as_ref().is_ok_and(|s| !s.success()) {
        println!("{} 'paccache' failed or was not found.", "✗".red());
        println!(
            "  ensure {} is installed (run {}).",
            "pacman-contrib".bold(),
            "haj i pacman-contrib".cyan()
        );
        return;
    }

    if let Some(home) = std::env::var("HOME").map(std::path::PathBuf::from).ok() {
        let aur_cache = home.join(".cache/haj/aur");
        if aur_cache.exists()
            && let Ok(entries) = std::fs::read_dir(&aur_cache)
        {
            for entry in entries.filter_map(Result::ok) {
                if entry.path().is_dir() {
                    let _ = Command::new("paccache")
                        .args(["-r", "-k", &keep_str, "-c", entry.path().to_str().unwrap()])
                        .output();
                }
            }
        }
    }

    println!("\n{} cache scrubbed successfully.", "✓".green());
}
