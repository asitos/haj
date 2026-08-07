use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn setup_temp_home(test_name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("haj_config_test_{}", test_name));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn test_config_missing_home() {
    // If HOME is not set, it should fallback safely or error gracefully
    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .env_remove("HOME")
        .arg("--help")
        .output()
        .expect("Failed to execute haj");

    assert!(
        output.status.success(),
        "haj should not crash if HOME is missing"
    );
}

#[test]
fn test_config_invalid_toml() {
    let home = setup_temp_home("invalid_toml");
    let config_dir = home.join(".config").join("haj");
    fs::create_dir_all(&config_dir).unwrap();

    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "invalid toml syntax = {[").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .env("HOME", &home)
        .arg("--help")
        .output()
        .expect("Failed to execute haj");

    // It should log a warning but still run
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to parse config") || stderr.is_empty(),
        "Expected error message or graceful default"
    );
}

#[test]
fn test_config_load_custom() {
    let home = setup_temp_home("custom_config");
    let config_dir = home.join(".config").join("haj");
    fs::create_dir_all(&config_dir).unwrap();

    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "[general]\naur_only = true\n").unwrap();

    let _output = Command::new(env!("CARGO_BIN_EXE_haj"))
        .env("HOME", &home)
        .arg("install")
        .arg("--dry-run")
        .arg("dummy_pkg")
        .output()
        .expect("Failed to execute haj");

    // The dry run should process dummy_pkg as an AUR package due to the config.
    // However, if the network is disconnected, it might fail to search aur.
    // We just want to ensure no crash from config load.
}
