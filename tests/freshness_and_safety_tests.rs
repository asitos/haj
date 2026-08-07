use std::process::Command;
use std::fs;
use std::path::PathBuf;

fn setup_temp_root(test_name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("haj_test_{}", test_name));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn test_freshness_missing_db() {
    let root_path = setup_temp_root("missing_db");
    
    // Create necessary sync dir
    let sync_dir = root_path.join("var/lib/pacman/sync");
    fs::create_dir_all(&sync_dir).unwrap();
    
    let mut path_env = std::env::var("PATH").unwrap_or_default();
    path_env = format!("{}:{}", root_path.display(), path_env);
    
    let fake_sudo = root_path.join("sudo");
    fs::write(&fake_sudo, "#!/bin/sh\necho \"fake sudo\"\nexit 0").unwrap();
    std::process::Command::new("chmod").args(["+x", fake_sudo.to_str().unwrap()]).status().unwrap();

    let child = Command::new(env!("CARGO_BIN_EXE_haj"))
        .env("PATH", path_env)
        .arg("--root")
        .arg(&root_path)
        .arg("--noconfirm")
        .arg("search")
        .arg("dummy_pkg")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
        
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    assert!(stdout.contains("package databases are missing."));
}

#[test]
fn test_freshness_stale_db() {
    let root_path = setup_temp_root("stale_db");
    
    let sync_dir = root_path.join("var/lib/pacman/sync");
    fs::create_dir_all(&sync_dir).unwrap();
    
    let db_path = sync_dir.join("core.db");
    fs::write(&db_path, "dummy data").unwrap();
    
    // Set file mtime to 8 days ago using touch
    let _ = Command::new("touch")
        .arg("-d")
        .arg("8 days ago")
        .arg(&db_path)
        .status()
        .unwrap();
    
    let mut path_env = std::env::var("PATH").unwrap_or_default();
    path_env = format!("{}:{}", root_path.display(), path_env);
    
    let fake_sudo = root_path.join("sudo");
    fs::write(&fake_sudo, "#!/bin/sh\necho \"fake sudo\"\nexit 0").unwrap();
    std::process::Command::new("chmod").args(["+x", fake_sudo.to_str().unwrap()]).status().unwrap();

    let child = Command::new(env!("CARGO_BIN_EXE_haj"))
        .env("PATH", path_env)
        .arg("--root")
        .arg(&root_path)
        .arg("--noconfirm")
        .arg("search")
        .arg("dummy_pkg")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
        
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    assert!(stdout.contains("package databases are stale (older than 7 days)."));
}
