use crossterm::style::Stylize;
use std::cmp::Ordering;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Extracts (`package_name`, `version_string`) from standard Arch package filenames
/// e.g. "bash-5.2.026-1-x86_64.pkg.tar.zst" -> ("bash", "5.2.026-1")
fn parse_package_filename(filename: &str) -> Option<(&str, &str)> {
    let clean = filename
        .strip_suffix(".pkg.tar.zst")
        .or_else(|| filename.strip_suffix(".pkg.tar.xz"))?;

    let parts: Vec<&str> = clean.split('-').collect();
    if parts.len() < 4 {
        return None;
    }

    // arch package naming convention: <name>-<version>-<release>-<arch>
    let name_parts_count = parts.len() - 3;
    let name_len = parts[..name_parts_count].join("-").len();
    let pkg_name = &clean[..name_len];

    let version_start = name_len + 1;
    let arch_start = clean.rfind('-')?;
    let version_str = &clean[version_start..arch_start];

    Some((pkg_name, version_str))
}

pub fn select_downgrade_target(package: &str) -> Option<PathBuf> {
    println!(
        "{} searching caches for '{}'...",
        "::".blue(),
        package.bold()
    );

    let current_version = if let Ok(alpm) = crate::core::alpm_init::init_alpm() {
        alpm.localdb()
            .pkg(package)
            .map(|pkg| pkg.version().to_string())
            .ok()
    } else {
        None
    };

    if let Some(ref cur_ver) = current_version {
        println!(
            "{} currently installed: {} v{}",
            "::".blue(),
            package.bold(),
            cur_ver.clone().yellow()
        );
    }

    let mut candidate_entries: Vec<(PathBuf, String)> = Vec::new();

    let native_cache = Path::new("/var/cache/pacman/pkg");
    if let Ok(entries) = std::fs::read_dir(native_cache) {
        for entry in entries.filter_map(Result::ok) {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if let Some((parsed_name, version)) = parse_package_filename(&file_name)
                && parsed_name == package
            {
                candidate_entries.push((entry.path(), version.to_string()));
            }
        }
    }

    if let Ok(home) = std::env::var("HOME").map(std::path::PathBuf::from) {
        let aur_cache = home.join(format!(".cache/haj/aur/{package}"));
        if let Ok(entries) = std::fs::read_dir(&aur_cache) {
            for entry in entries.filter_map(Result::ok) {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if let Some((parsed_name, version)) = parse_package_filename(&file_name)
                    && parsed_name == package
                {
                    candidate_entries.push((entry.path(), version.to_string()));
                }
            }
        }
    }

    if candidate_entries.is_empty() {
        println!(
            "{} no cached versions found for '{}'.",
            "✗".red(),
            package.bold()
        );
        return None;
    }

    candidate_entries.sort_by(|a, b| {
        let cmp = alpm::vercmp(a.1.as_str(), b.1.as_str());
        match cmp {
            Ordering::Greater => Ordering::Less,
            Ordering::Less => Ordering::Greater,
            Ordering::Equal => Ordering::Equal,
        }
    });

    println!("{}", "\navailable versions:".bold().white());
    for (i, (path, ver)) in candidate_entries.iter().enumerate() {
        let is_aur = path.to_string_lossy().contains(".cache/haj");
        let tag = if is_aur { " (aur)" } else { "" };
        let status_tag = if let Some(ref cur_ver) = current_version {
            let cmp = alpm::vercmp(ver.as_str(), cur_ver.as_str());
            match cmp {
                Ordering::Equal => " [current]".yellow().to_string(),
                Ordering::Less => " [downgrade]".cyan().to_string(),
                Ordering::Greater => " [upgrade]".green().to_string(),
            }
        } else {
            String::new()
        };
        println!(
            "  {}) {} v{}{}{}",
            (i + 1).to_string().cyan(),
            package.magenta().bold(),
            ver.clone().green(),
            status_tag,
            tag.dim()
        );
    }

    print!(
        "\n{} select a version [1-{}]: ",
        "❓".magenta().bold(),
        candidate_entries.len()
    );
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);

    if let Ok(idx) = input.trim().parse::<usize>()
        && idx > 0
        && idx <= candidate_entries.len()
    {
        return Some(candidate_entries[idx - 1].0.clone());
    }

    println!("{} invalid selection or aborted.", "✗".red());
    None
}
