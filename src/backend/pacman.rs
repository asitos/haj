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

    async fn search(&self, _query: &str) -> Result<Vec<PackageModel>, BackendError> {
        todo!()
    }

    async fn info(&self, _package: &str) -> Result<Option<PackageModel>, BackendError> {
        todo!()
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
