use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct HajConfig {
    pub aur_only: bool,
    pub repo_only: bool,
    pub verbose: bool,
    pub diff_prog: String,
    pub build_dir: String,
    pub animations: bool,
}

impl Default for HajConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            aur_only: false,
            repo_only: false,
            verbose: false,
            diff_prog: "vimdiff".to_string(),
            build_dir: home.join(".cache/haj/aur").to_string_lossy().to_string(),
            animations: true,
        }
    }
}

pub fn load_config() -> HajConfig {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join(".config")
        })
        .join("haj");

    let config_path = config_dir.join("config.toml");

    if !config_path.exists() {
        let _ = fs::create_dir_all(&config_dir);
        let default_config = HajConfig::default();
        
        let toml_string = format!(
            "# haj package manager configuration\n\n{}",
            toml::to_string_pretty(&default_config).unwrap_or_default()
        );
        
        let _ = fs::write(&config_path, toml_string);
        return default_config;
    }

    match fs::read_to_string(&config_path) {
        Ok(contents) => match toml::from_str(&contents) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("\x1b[31m✗ failed to parse config.toml: {}\x1b[0m", e);
                HajConfig::default()
            }
        },
        Err(_) => HajConfig::default(),
    }
}
