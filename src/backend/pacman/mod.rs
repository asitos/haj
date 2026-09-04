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
                    || pkg
                        .desc()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query.to_lowercase())
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

    fn build_install_local(&self, path: &std::path::Path) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec![
                "-U".to_string(),
                path.to_string_lossy().to_string(),
                "--noconfirm".to_string(),
            ],
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

    async fn get_all_packages(&self) -> Result<(Vec<PackageModel>, usize, usize), BackendError> {
        let alpm_handle = crate::core::alpm_init::init_alpm()
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
        Ok(crate::core::pacman::get_all_packages(&alpm_handle))
    }

    async fn get_install_summaries(
        &self,
        packages: &[&str],
    ) -> Result<Vec<crate::backend::traits::InstallSummary>, BackendError> {
        let alpm_handle = crate::core::alpm_init::init_alpm()
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
        let pkgs = packages.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let summaries = crate::core::get_install_summaries(&alpm_handle, &pkgs)
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
        Ok(summaries
            .into_iter()
            .map(|s| crate::backend::traits::InstallSummary {
                name: s.name,
                version: s.version,
                download_size_mb: s.download_size_mb,
                install_size_mb: s.install_size_mb,
            })
            .collect())
    }

    async fn detect_conflicts(
        &self,
        packages: &[&str],
    ) -> Result<Vec<crate::backend::traits::ConflictInfo>, BackendError> {
        let alpm_handle = crate::core::alpm_init::init_alpm()
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
        let pkgs = packages.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let conflicts = crate::core::conflicts::detect_conflicts(
            &alpm_handle,
            &pkgs,
            &std::collections::HashMap::new(),
        );
        Ok(conflicts
            .into_iter()
            .map(|c| crate::backend::traits::ConflictInfo {
                incoming_pkg: c.incoming_pkg,
                installed_pkg: c.installed_pkg,
                constraint: c.constraint,
            })
            .collect())
    }

    async fn resolve_aur(
        &self,
        packages: &[&str],
    ) -> Result<crate::backend::traits::AurResolution, BackendError> {
        let mut resolved = Vec::new();
        for &pkg in packages {
            let res = crate::core::aur::search(pkg)
                .await
                .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
            let mut found = false;
            for r in res.results {
                if r.name == pkg {
                    let alpm_handle = crate::core::alpm_init::init_alpm().ok();
                    let local_ver = alpm_handle
                        .as_ref()
                        .and_then(|h| h.localdb().pkg(pkg).map(|p| p.version().to_string()).ok());
                    let is_update = local_ver
                        .as_ref()
                        .is_some_and(|lv| crate::core::aur::is_newer(&r.version, lv));
                    resolved.push((r.name, r.version, is_update, local_ver));
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(BackendError::NotFound(pkg.to_string()));
            }
        }
        Ok(crate::backend::traits::AurResolution { packages: resolved })
    }

    async fn build_aur_package(
        &self,
        package: &str,
        verbose: bool,
    ) -> Result<std::path::PathBuf, BackendError> {
        crate::core::aur::build(package, verbose)
            .await
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))
    }
}
