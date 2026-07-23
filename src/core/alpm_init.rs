use alpm::{Alpm, SigLevel};
use anyhow::{Context, Result};

pub fn init_alpm() -> Result<Alpm> {
    let root_path = "/";
    let db_path = "/var/lib/pacman/";

    let handle = Alpm::new(root_path, db_path)
        .with_context(|| format!("Failed to initialize ALPM at root: {} and db: {}", root_path, db_path))?;

    let repos = ["core", "extra", "multilib"];
    for repo in repos {
        handle.register_syncdb(repo, SigLevel::USE_DEFAULT).ok();
    }

    Ok(handle)
}
