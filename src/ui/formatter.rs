use alpm::Package;
use owo_colors::OwoColorize;

pub fn print_search_result(pkg: &Package, repo_name: &str, is_installed: bool) {
    let installed_str = if is_installed {
        "Yes".green().bold().to_string()
    } else {
        "No".dimmed().to_string()
    };

    let size_mb = pkg.isize() as f64 / 1024.0 / 1024.0;

    // The Cargo/Bun aesthetic
    println!("{}", pkg.name().bold().white());
    println!("{}", pkg.desc().unwrap_or("no description").dimmed());
    println!();
    println!("{:<12} {}", "version:", pkg.version().cyan());
    println!("{:<12} {}", "repository:", repo_name.magenta());
    println!("{:<12} {}", "installed:", installed_str);
    println!("{:<12} {:.2} MB\n", "size:", size_mb);
}
