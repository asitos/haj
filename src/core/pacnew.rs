use owo_colors::OwoColorize;
use std::io::Write;
use std::process::{Command, Stdio};

pub fn manage_pacnew_files() {
    println!("{} scanning for .pacnew files...", "::".blue());

    let output = Command::new("sudo")
        .arg("find")
        .arg("/etc")
        .arg("-type")
        .arg("f")
        .arg("-name")
        .arg("*.pacnew")
        .output()
        .expect("failed to search for pacnew files");

    let files_str = String::from_utf8_lossy(&output.stdout);
    let pacnew_files: Vec<&str> = files_str.lines().filter(|l| !l.trim().is_empty()).collect();

    if pacnew_files.is_empty() {
        println!("{} no .pacnew files found. system is clean!", "✓".green());
        return;
    }

    println!(
        "{} found {} pending merge(s):\n",
        "✓".green(),
        pacnew_files.len().to_string().cyan()
    );

    let config = crate::config::load_config();

    let editor = std::env::var("MERGEPROG")
        .or_else(|_| std::env::var("DIFFPROG"))
        .unwrap_or(config.general.diff_prog);

    for pacnew in pacnew_files {
        let original = pacnew.trim_end_matches(".pacnew");
        println!("  {} {}", "•".magenta(), original.bold());
        println!("    └─ {}", pacnew.dimmed());

        print!(
            "\n{} merge these files now? [Y/n/q (quit)] ",
            "?".magenta().bold()
        );
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let ans = input.trim().to_lowercase();

        if ans == "q" {
            println!("{} aborted.", "✗".red());
            break;
        } else if ans.is_empty() || ans == "y" {
            let mut child = Command::new("sudo")
                .arg(&editor)
                .arg(original)
                .arg(pacnew)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("failed to launch diff editor");

            let _ = child.wait();

            print!(
                "\n{} delete {}? [Y/n] ",
                "?".magenta().bold(),
                pacnew.dimmed()
            );
            let _ = std::io::stdout().flush();

            let mut del_input = String::new();
            std::io::stdin().read_line(&mut del_input).unwrap();
            let del_ans = del_input.trim().to_lowercase();

            if del_ans.is_empty() || del_ans == "y" {
                Command::new("sudo")
                    .arg("rm")
                    .arg(pacnew)
                    .status()
                    .expect("failed to delete pacnew");
                println!("{} deleted {}.", "✓".green(), pacnew);
            }
        }
        println!();
    }

    println!("{} pacnew management complete.", "✓".green());
}
