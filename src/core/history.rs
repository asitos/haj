use owo_colors::OwoColorize;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn show_history(limit: usize) {
    let file = match File::open("/var/log/pacman.log") {
        Ok(f) => f,
        Err(e) => {
            println!("{} failed to open pacman.log: {}", "✗".red(), e);
            return;
        }
    };

    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        if line.contains("[ALPM] installed")
            || line.contains("[ALPM] upgraded")
            || line.contains("[ALPM] removed")
            || line.contains("[ALPM] downgraded")
        {
            entries.push(line);
        }
    }

    if entries.is_empty() {
        println!("{} no transaction history found.", "::".blue());
        return;
    }

    let start = if entries.len() > limit { entries.len() - limit } else { 0 };
    println!("{} showing last {} transactions:\n", "::".blue().bold(), limit.to_string().cyan());

    for entry in &entries[start..] {
        let parts: Vec<&str> = entry.splitn(2, " [ALPM] ").collect();

        if parts.len() == 2 {
            let timestamp = parts[0].replace('[', "").replace(']', "");
            let action_string = parts[1];

            let colored_action = if action_string.starts_with("installed") {
                action_string.replace("installed", &"installed".green().bold().to_string())
            } else if action_string.starts_with("upgraded") {
                action_string.replace("upgraded", &"upgraded".yellow().bold().to_string())
            } else if action_string.starts_with("removed") {
                action_string.replace("removed", &"removed".red().bold().to_string())
            } else if action_string.starts_with("downgraded") {
                action_string.replace("downgraded", &"downgraded".magenta().bold().to_string())
            } else {
                action_string.to_string()
            };

            println!("  {} {}", timestamp.dimmed(), colored_action);
        } else {
            println!("  {}", entry);
        }
    }
}
