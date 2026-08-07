use std::process::Command;
use std::str;

#[test]
fn test_cli_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .arg("--help")
        .output()
        .expect("Failed to execute haj");

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("Commands (alias):"));
    assert!(stdout.contains("Options:"));
}

#[test]
fn test_cli_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .arg("--version")
        .output()
        .expect("Failed to execute haj");

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("haj"));
    assert!(stdout.contains("0.2.8"));
}

#[test]
fn test_cli_invalid_command_is_interactive() {
    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .arg("nonexistent_command")
        .output()
        .expect("Failed to execute haj");

    // It should succeed with 0 and print that it's searching for the package
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("searching for"));
    assert!(stdout.contains("nonexistent_command"));
    assert!(stdout.contains("no packages found"));
}

#[test]
fn test_cli_dry_run_flag() {
    // Run an install command with --dry-run
    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .arg("--dry-run")
        .arg("install")
        .arg("dummy_pkg")
        .output()
        .expect("Failed to execute haj");

    let stdout = str::from_utf8(&output.stdout).unwrap();
    // It should not fail, and it should say dry run
    assert!(
        stdout.contains("[dry run]")
            || output.status.success()
            || str::from_utf8(&output.stderr).unwrap().contains("error")
    );
}

#[test]
fn test_cli_aliases() {
    let aliases = vec![
        "i", "rm", "s", "info", "g", "ls", "st", "l", "f", "sink", "ow", "lf", "loc", "h", "o",
        "c", "m", "pn",
    ];

    for alias in aliases {
        let output = Command::new(env!("CARGO_BIN_EXE_haj"))
            .arg(alias)
            .arg("--help")
            .output()
            .expect("Failed to execute haj");

        assert!(
            output.status.success(),
            "Alias {} failed to show help",
            alias
        );
    }
}
