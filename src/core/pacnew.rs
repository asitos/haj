use owo_colors::OwoColorize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn manage_pacnew_files() {
    println!("{} scanning /etc/ for .pacnew files...\n", "✓".green());

    let mut pacnew_files = Vec::new();
    scan_directory(Path::new("/etc"), &mut pacnew_files);

    if pacnew_files.is_empty() {
        println!(
            "{} no .pacnew files found. your configs are clean!",
            "✓".green()
        );
        return;
    }

    println!(
        "{} found {} config updates needing attention.\n",
        "!".yellow(),
        pacnew_files.len().bold()
    );

    for pacnew in pacnew_files {
        let original = pacnew.with_extension(""); // strips the .pacnew extension

        println!("{}", "=".repeat(50).dimmed());
        println!("{} {}", "target:".cyan(), original.display());
        println!("{} {}\n", "new:".magenta(), pacnew.display());

        loop {
            print!("action [ (v)iew diff | (o)verwrite | (r)emove new | (s)kip ]: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let choice = input.trim().to_lowercase();

            match choice.as_str() {
                "v" => {
                    let status = Command::new("delta").arg(&original).arg(&pacnew).status();

                    if status.is_err() {
                        Command::new("diff")
                            .arg("--color=always")
                            .arg("-u")
                            .arg(&original)
                            .arg(&pacnew)
                            .status()
                            .ok();
                    }
                }
                "o" => match fs::rename(&pacnew, &original) {
                    Ok(_) => {
                        println!("{} overwrote original with new config.", "✓".green());
                        break;
                    }
                    Err(e) => println!("{} failed to overwrite: {}", "✗".red(), e),
                },
                "r" => match fs::remove_file(&pacnew) {
                    Ok(_) => {
                        println!("{} deleted .pacnew file.", "✓".green());
                        break;
                    }
                    Err(e) => println!("{} failed to delete: {}", "✗".red(), e),
                },
                "s" | "" => {
                    println!("{} skipped.", "→".cyan());
                    break;
                }
                _ => println!("{} invalid choice.", "✗".red()),
            }
        }
    }

    println!("\n{} all caught up!", "✓".green());
}

fn scan_directory(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_directory(&path, files);
            } else if path.extension().and_then(|s| s.to_str()) == Some("pacnew") {
                files.push(path);
            }
        }
    }
}
