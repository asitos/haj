use alpm::{Alpm, SigLevel};
use anyhow::{Context, Result};
use std::path::Path;

pub fn init_alpm() -> Result<Alpm> {
    let root_path = "/";
    let db_path = "/var/lib/pacman/";

    let handle = Alpm::new(root_path, db_path).with_context(|| {
        format!(
            "failed to initialize alpm at root: {} and db: {}",
            root_path, db_path
        )
    })?;

    let repos = ["core", "extra", "multilib"];
    for repo in repos {
        handle.register_syncdb(repo, SigLevel::USE_DEFAULT).ok();
    }

    Ok(handle)
}

pub fn owns(alpm: &Alpm, file_path: &str) {
    let local_db = alpm.localdb();
    let target = Path::new(file_path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(file_path).to_path_buf());
    let target_str = target.to_string_lossy();

    let mut found = false;
    for pkg in local_db.pkgs() {
        let clean_target = target_str.trim_start_matches('/');

        if pkg.files().contains(clean_target).is_some() {
            println!("{} is owned by {} {}", file_path, pkg.name(), pkg.version());
            found = true;
            break;
        }
    }

    if !found {
        println!("❌ no package owns {}", file_path);
    }
}

pub fn files(alpm: &Alpm, pkg_name: &str) {
    let local_db = alpm.localdb();

    match local_db.pkg(pkg_name) {
        Ok(pkg) => {
            println!("files for {}:", pkg.name());
            for file in pkg.files().files() {
                let file_name = String::from_utf8_lossy(file.name());
                println!("  /{}", file_name);
            }
        }
        Err(_) => println!("❌ package '{}' not found in local database.", pkg_name),
    }
}
