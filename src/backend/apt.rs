use crate::backend::traits::{BackendError, Capabilities, CommandPlan, PackageManager};
use crate::core::package::PackageModel;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

pub struct AptBackend {}

impl AptBackend {
    pub fn new() -> Result<Self, BackendError> {
        Ok(Self {})
    }
}

#[async_trait]
impl PackageManager for AptBackend {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_aur: false,
            supports_downgrade: true,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<PackageModel>, BackendError> {
        let output = Command::new("apt-cache")
            .arg("search")
            .arg(query)
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;

        if !output.status.success() {
            return Err(BackendError::ExecutionError(
                output.status.code().unwrap_or(1),
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let mut results = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some((name, _desc)) = line.split_once(" - ") {
                results.push(PackageModel {
                    name: name.trim().to_string(),
                    version: "Unknown".to_string(),
                    repo: "apt".to_string(),
                    is_installed: false,
                    is_upgradable: false,
                    size_mb: 0.0,
                });
            }
        }

        Ok(results)
    }

    async fn info(&self, package: &str) -> Result<Option<PackageModel>, BackendError> {
        let output = Command::new("apt-cache")
            .arg("show")
            .arg(package)
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut version = String::new();
        let mut size_mb = 0.0;

        for line in stdout.lines() {
            if let Some(v) = line.strip_prefix("Version: ") {
                if version.is_empty() {
                    version = v.trim().to_string();
                }
            } else if let Some(s) = line.strip_prefix("Size: ") {
                if let Ok(bytes) = s.trim().parse::<f64>() {
                    size_mb = bytes / 1_048_576.0;
                }
            }
        }

        Ok(Some(PackageModel {
            name: package.to_string(),
            version,
            repo: "apt".to_string(),
            is_installed: false,
            is_upgradable: false,
            size_mb,
        }))
    }

    async fn list_installed(&self) -> Result<Vec<PackageModel>, BackendError> {
        let output = Command::new("dpkg-query")
            .arg("-W")
            .arg("-f=${binary:Package}|${Version}|${Installed-Size}\\n")
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;

        if !output.status.success() {
            return Err(BackendError::ExecutionError(
                output.status.code().unwrap_or(1),
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let mut results = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                let name = parts[0].to_string();
                let version = parts[1].to_string();
                let size_kb = parts[2].parse::<f64>().unwrap_or(0.0);
                
                results.push(PackageModel {
                    name,
                    version,
                    repo: "local".to_string(),
                    is_installed: true,
                    is_upgradable: false, // We'd need to parse apt list --upgradable for this
                    size_mb: size_kb / 1024.0,
                });
            }
        }

        Ok(results)
    }

    fn build_install(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "apt-get".to_string(),
            args: vec!["install".to_string(), "-y".to_string()]
                .into_iter()
                .chain(packages.iter().map(|s| s.to_string()))
                .collect(),
            requires_root: true,
        })
    }

    fn build_remove(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "apt-get".to_string(),
            args: vec!["remove".to_string(), "--auto-remove".to_string(), "-y".to_string()]
                .into_iter()
                .chain(packages.iter().map(|s| s.to_string()))
                .collect(),
            requires_root: true,
        })
    }

    fn build_update(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "apt-get".to_string(),
            args: vec!["update".to_string()],
            requires_root: true,
        })
    }

    fn build_upgrade(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "apt-get".to_string(),
            args: vec!["upgrade".to_string(), "-y".to_string()],
            requires_root: true,
        })
    }
}
