use crossterm::style::Stylize;
use std::process::{Command, Stdio};

pub fn manage_pacnew_files() {
    println!("{} launching pacdiff...", "::".blue());

    let mut child = Command::new("sudo")
        .arg("pacdiff")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to launch pacdiff. is pacman-contrib installed?");

    let status = child.wait().expect("failed to wait on pacdiff");

    if status.success() {
        println!("{} pacnew management complete.", "✓".green());
    } else {
        println!("{} pacnew management aborted or failed.", "✗".red());
    }
}
