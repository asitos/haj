#![allow(dead_code)]
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct HajConfig {
    pub general: GeneralConfig,
}

#[derive(Deserialize, Debug)]
pub struct GeneralConfig {
    pub parallel_downloads: u8,
    pub animations: bool,
    pub color: String,
    pub verbose: bool,
}

impl Default for HajConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                parallel_downloads: 10,
                animations: true,
                color: "auto".to_string(),
                verbose: false,
            },
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        HajConfig::default().general
    }
}

pub fn load_config() -> HajConfig {
    let config_path = Path::new("/etc/haj.conf");

    if config_path.exists() {
        if let Ok(contents) = fs::read_to_string(config_path) {
            if let Ok(config) = toml::from_str(&contents) {
                return config;
            }
        }
    }

    HajConfig::default()
}
