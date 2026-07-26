use owo_colors::OwoColorize;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn select_downgrade_target(package: &str) -> Option<PathBuf> {
    println!("{} searching caches for '{}'...", "::".blue(), package.bold());
    
    let mut candidates = Vec::new();

    let native_cache = Path::new("/var/cache/pacman/pkg");
    if let Ok(entries) = std::fs::read_dir(native_cache) {
        for entry in entries.filter_map(Result::ok) {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with(&format!("{}-", package)) 
                && (file_name.ends_with(".pkg.tar.zst") || file_name.ends_with(".pkg.tar.xz")) 
            {
                candidates.push(entry.path());
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let aur_cache = home.join(format!(".cache/haj/aur/{}", package));
        if let Ok(entries) = std::fs::read_dir(aur_cache) {
            for entry in entries.filter_map(Result::ok) {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with(&format!("{}-", package)) && file_name.ends_with(".pkg.tar.zst") {
                    candidates.push(entry.path());
                }
            }
        }
    }

    if candidates.is_empty() {
        println!("{} no cached versions found for '{}'.", "✗".red(), package.bold());
        return None;
    }

    // sort by new for now
    candidates.sort_by_key(|a| std::fs::metadata(a).and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH));
    candidates.reverse();

    println!("{}", "\navailable versions:".bold().white());
    for (i, path) in candidates.iter().enumerate() {
        let name = path.file_name().unwrap().to_string_lossy();
        let tag = if path.to_string_lossy().contains(".cache/haj") { " (aur)" } else { "" };
        println!("  {}) {}{}", (i + 1).to_string().cyan(), name.magenta(), tag.dimmed());
    }

    print!("\n{} select a version [1-{}]: ", "?".magenta().bold(), candidates.len());
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);

    if let Ok(idx) = input.trim().parse::<usize>() {
        if idx > 0 && idx <= candidates.len() {
            return Some(candidates[idx - 1].clone());
        }
    }
    
    println!("{} invalid selection or aborted.", "✗".red());
    None
}
