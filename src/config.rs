use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
// use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub animations: bool,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            animations: true, // default spinning
            theme: "catppuccin".to_string(),
        }
    }
}

pub fn load_config() -> AppConfig {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "haj") {
        let config_dir = proj_dirs.config_dir();
        let config_file = config_dir.join("config.toml");

        if config_file.exists() {
            if let Ok(contents) = fs::read_to_string(&config_file) {
                if let Ok(config) = toml::from_str(&contents) {
                    return config;
                }
            }
        } else {
            let _ = fs::create_dir_all(config_dir);
            let default_config = AppConfig::default();
            if let Ok(toml_string) = toml::to_string_pretty(&default_config) {
                let _ = fs::write(config_file, toml_string);
            }
            return default_config;
        }
    }

    // fallback
    AppConfig::default()
}
