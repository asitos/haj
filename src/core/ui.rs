use crate::core;
use crate::ui;
use crossterm::style::Stylize;
use std::io::Write;
pub fn prompt_confirm(msg: &str) -> bool {
    print!("{} {} ", "?".magenta().bold(), msg.bold());
    let _ = std::io::stdout().flush();

    if crossterm::terminal::enable_raw_mode().is_ok() {
        let result;
        loop {
            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && key.code == crossterm::event::KeyCode::Char('c')
                {
                    result = false;
                    break;
                }
                match key.code {
                    crossterm::event::KeyCode::Char('y')
                    | crossterm::event::KeyCode::Char('Y')
                    | crossterm::event::KeyCode::Enter => {
                        result = true;
                        break;
                    }
                    crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Char('N') => {
                        result = false;
                        break;
                    }
                    _ => continue,
                }
            }
        }
        let _ = crossterm::terminal::disable_raw_mode();
        println!("{}", if result { "Y" } else { "n" });
        result
    } else {
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        let input = input.trim().to_lowercase();
        input.is_empty() || input == "y"
    }
}

pub enum InstallChoice {
    Yes,
    No,
    View,
}

pub fn prompt_install(msg: &str) -> InstallChoice {
    print!("{} {} ", "?".magenta().bold(), msg.bold());
    let _ = std::io::stdout().flush();

    if crossterm::terminal::enable_raw_mode().is_ok() {
        let result;
        loop {
            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && key.code == crossterm::event::KeyCode::Char('c')
                {
                    result = InstallChoice::No;
                    break;
                }
                match key.code {
                    crossterm::event::KeyCode::Char('y')
                    | crossterm::event::KeyCode::Char('Y')
                    | crossterm::event::KeyCode::Enter => {
                        result = InstallChoice::Yes;
                        break;
                    }
                    crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Char('N') => {
                        result = InstallChoice::No;
                        break;
                    }
                    crossterm::event::KeyCode::Char('v') | crossterm::event::KeyCode::Char('V') => {
                        result = InstallChoice::View;
                        break;
                    }
                    _ => continue,
                }
            }
        }
        let _ = crossterm::terminal::disable_raw_mode();
        let display_str = match result {
            InstallChoice::Yes => "Y",
            InstallChoice::No => "n",
            InstallChoice::View => "v",
        };
        println!("{}", display_str);
        result
    } else {
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        let input = input.trim().to_lowercase();
        if input == "v" {
            InstallChoice::View
        } else if input.is_empty() || input == "y" {
            InstallChoice::Yes
        } else {
            InstallChoice::No
        }
    }
}

pub async fn display_arch_news() {
    let spinner = ui::spinner("checking arch linux news...");
    let url = "https://archlinux.org/feeds/news/";

    if let Ok(response) = reqwest::get(url).await {
        if let Ok(xml) = response.text().await {
            spinner.finish_and_clear();

            if let Some(item_start) = xml.find("<item>") {
                let item_str = &xml[item_start..];

                if let (Some(t_start), Some(t_end)) =
                    (item_str.find("<title>"), item_str.find("</title>"))
                {
                    let title = &item_str[t_start + 7..t_end];

                    if let (Some(d_start), Some(d_end)) =
                        (item_str.find("<pubDate>"), item_str.find("</pubDate>"))
                    {
                        let date_str = &item_str[d_start + 9..d_end];

                        if let Ok(pub_date) = chrono::DateTime::parse_from_rfc2822(date_str) {
                            let now = chrono::Utc::now();
                            if now
                                .signed_duration_since(pub_date.with_timezone(&chrono::Utc))
                                .num_days()
                                <= 7
                            {
                                println!(
                                    "\n{} {}\n  {} {}\n  {}\n",
                                    "!!!".red().bold(),
                                    "action required: recent arch linux news".red().bold(),
                                    "headline:".dim(),
                                    title.yellow().bold(),
                                    "haj requests you to read the news before upgrading.".dim()
                                );
                            }
                        }
                    }
                }
            }
        } else {
            spinner.finish_and_clear();
        }
    } else {
        spinner.finish_and_clear();
    }
}

pub fn handle_conflicts_ui<F>(
    conflicts: &[core::conflicts::ConflictInfo],
    dry_run: bool,
    noconfirm: bool,
    mut prompter: F,
) -> Result<bool, &'static str>
where
    F: FnMut(&str) -> bool,
{
    if conflicts.is_empty() {
        return Ok(false);
    }
    for conflict in conflicts {
        println!("\n{} package conflict detected\n", "✗".red());
        println!("  installing: {}", conflict.incoming_pkg.clone().bold());
        println!(
            "  conflicts with installed package: {}",
            conflict.installed_pkg.clone().bold()
        );
        if let Some(constraint) = &conflict.constraint {
            println!("  constraint: {}", constraint.clone().yellow());
        }
        println!("\n  only one of these packages can be installed.\n");

        if dry_run {
            println!(
                "{} would replace '{}' with '{}'",
                "[dry run]".bold().yellow(),
                conflict.installed_pkg,
                conflict.incoming_pkg
            );
        } else if noconfirm {
            return Err(
                "conflicting packages detected and --noconfirm used without explicit authorization. aborting.",
            );
        } else {
            let prompt = format!(
                "replace installed package '{}' with '{}'? [y/N]",
                conflict.installed_pkg, conflict.incoming_pkg
            );
            if !prompter(&prompt) {
                return Err("aborted: no changes were made.");
            }
        }
    }
    Ok(true)
}
