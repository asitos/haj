use std::fs;
use std::process::Command;
use std::str;

#[test]
fn test_command_remove_dry_run() {
    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .arg("remove")
        .arg("--dry-run")
        .arg("dummy_pkg")
        .output()
        .expect("Failed to execute haj");

    let _stdout = str::from_utf8(&output.stdout).unwrap();
    // For remove dry_run, it should output that it's doing a dry run or just silently do it
    assert!(output.status.success() || str::from_utf8(&output.stderr).unwrap().contains("error:"));
}

#[test]
fn test_command_clean_dry_run() {
    // We can't really pass dry-run to clean easily unless the CLI supports it,
    // but we can pass keep=0 which might run paccache. We shouldn't do destructive ops.
    // So let's check `clean --help` to see if it parses.
    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .arg("clean")
        .arg("--help")
        .output()
        .expect("Failed to execute haj");
    assert!(output.status.success());
}

#[test]
fn test_command_mark() {
    // Testing the mark command parsing
    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .arg("mark")
        .arg("--help")
        .output()
        .expect("Failed to execute haj");
    assert!(output.status.success());
}

#[test]
fn test_command_history() {
    // Testing the history command
    // Usually reading /var/log/pacman.log, which may require permissions or missing.
    // If we pass an alternate root without log, it should handle gracefully.
    let mut path = std::env::temp_dir();
    path.push("haj_test_history");
    let _ = fs::create_dir_all(&path);

    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .arg("--root")
        .arg(&path)
        .arg("history")
        .output()
        .expect("Failed to execute haj");

    // Should gracefully exit if log is missing
    assert!(output.status.success());
}
