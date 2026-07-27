use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub aur_only: bool,
    pub repo_only: bool,
    pub verbose: bool,
    pub diff_prog: String,
    pub build_dir: String,
    pub animations: bool,
}

impl Default for GeneralConfig {
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

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HajConfig {
    pub general: GeneralConfig,
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
        if let Err(e) = fs::create_dir_all(&config_dir) {
            eprintln!("\x1b[31m✗ failed to create config directory: {}\x1b[0m", e);
            return HajConfig::default();
        }
        
        let default_config = HajConfig::default();
        
        let toml_string = format!(
            "# haj package manager configuration\n\n{}",
            toml::to_string_pretty(&default_config).unwrap_or_default()
        );
        
        if let Err(e) = fs::write(&config_path, toml_string) {
            eprintln!("\x1b[31m✗ failed to write config.toml: {}\x1b[0m", e);
        }
        
        return default_config;
    }

    match fs::read_to_string(&config_path) {
        Ok(contents) => match toml::from_str::<HajConfig>(&contents) {
            Ok(config) => {
                let updated_toml = format!(
                    "# haj package manager configuration\n\n{}",
                    toml::to_string_pretty(&config).unwrap_or_default()
                );
                
                if contents != updated_toml {
                    if let Err(e) = fs::write(&config_path, &updated_toml) {
                        eprintln!("\x1b[31m✗ failed to update config.toml: {}\x1b[0m", e);
                    }
                }
                
                config
            }
            Err(e) => {
                eprintln!("\x1b[31m✗ failed to parse config.toml: {}\x1b[0m", e);
                HajConfig::default()
            }
        },
        Err(_) => HajConfig::default(),
    }
}
