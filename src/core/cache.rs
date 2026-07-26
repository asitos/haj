use owo_colors::OwoColorize;
use std::process::Command;

pub fn scrub(keep: usize) {
    println!("{} scrubbing package cache (keeping last {} versions)...", "::".blue(), keep);

    let status = Command::new("paccache")
        .args(["-r", "-k", &keep.to_string()])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            // old aur cache cleanup
            if let Some(home) = dirs::home_dir() {
                let aur_cache = home.join(".cache/haj/aur");
                if aur_cache.exists() {
                    let _ = std::fs::remove_dir_all(&aur_cache);
                    let _ = std::fs::create_dir_all(&aur_cache);
                }
            }
            println!("\n{} cache scrubbed successfully.", "✓".green())
        },
        Ok(_) => println!("\n{} cache scrubber executed with warnings.", "!!!".yellow()),
        Err(_) => {
            println!("{} 'paccache' executable not found.", "✗".red());
            println!("  the {} package is required for native cache management.", "pacman-contrib".bold());
            println!("  run {} to install it.", "haj i pacman-contrib".cyan());
        }
    }
}
