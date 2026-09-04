use crate::backend::traits::{BackendError, Capabilities, CommandPlan, PackageManager};
use crate::core::package::PackageModel;
use async_trait::async_trait;

pub struct PacmanBackend {}

impl PacmanBackend {
    pub fn new() -> Result<Self, BackendError> {
        Ok(Self {})
    }
}

#[async_trait]
impl PackageManager for PacmanBackend {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_aur: true,
            supports_downgrade: true,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<PackageModel>, BackendError> {
        let alpm_handle = crate::core::alpm_init::init_alpm()
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
        let local_db = alpm_handle.localdb();
        let mut results = Vec::new();
        for db in alpm_handle.syncdbs() {
            for pkg in db.pkgs() {
                if pkg.name().to_lowercase().contains(&query.to_lowercase())
                    || pkg.desc().unwrap_or("").to_lowercase().contains(&query.to_lowercase())
                {
                    results.push(PackageModel {
                        name: pkg.name().to_string(),
                        version: pkg.version().to_string(),
                        repo: db.name().to_string(),
                        is_installed: local_db.pkg(pkg.name()).is_ok(),
                        is_upgradable: false,
                        size_mb: pkg.isize() as f64 / 1_048_576.0,
                    });
                }
            }
        }
        Ok(results)
    }

    async fn info(&self, package: &str) -> Result<Option<PackageModel>, BackendError> {
        let alpm_handle = crate::core::alpm_init::init_alpm()
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
        let local_db = alpm_handle.localdb();
        for db in alpm_handle.syncdbs() {
            if let Ok(pkg) = db.pkg(package) {
                return Ok(Some(PackageModel {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    repo: db.name().to_string(),
                    is_installed: local_db.pkg(pkg.name()).is_ok(),
                    is_upgradable: false,
                    size_mb: pkg.isize() as f64 / 1_048_576.0,
                }));
            }
        }
        Ok(None)
    }

    async fn list_installed(&self) -> Result<Vec<PackageModel>, BackendError> {
        let alpm_handle = crate::core::alpm_init::init_alpm()
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
        let (packages, _, _) = crate::core::pacman::get_all_packages(&alpm_handle);
        Ok(packages)
    }

    fn build_install(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-S".to_string(), "--noconfirm".to_string()]
                .into_iter()
                .chain(packages.iter().map(|s| s.to_string()))
                .collect(),
            requires_root: true,
        })
    }

    fn build_remove(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-Rs".to_string(), "--noconfirm".to_string()]
                .into_iter()
                .chain(packages.iter().map(|s| s.to_string()))
                .collect(),
            requires_root: true,
        })
    }

    fn build_update(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-Sy".to_string(), "--noconfirm".to_string()],
            requires_root: true,
        })
    }

    fn build_upgrade(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-Su".to_string(), "--noconfirm".to_string()],
            requires_root: true,
        })
    }
}
