use crate::core::package::PackageModel;
use async_trait::async_trait;

#[derive(Debug)]
pub enum BackendError {
    NotFound(String),
    NetworkError(String),
    ExecutionError(i32, String),
    ParseError(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(pkg) => write!(f, "Package not found: {}", pkg),
            Self::NetworkError(err) => write!(f, "Network error: {}", err),
            Self::ExecutionError(code, stderr) => {
                write!(f, "Execution failed ({}): {}", code, stderr)
            }
            Self::ParseError(err) => write!(f, "Parse error: {}", err),
        }
    }
}

impl std::error::Error for BackendError {}

#[derive(Clone, Debug)]
pub struct Capabilities {
    pub supports_aur: bool,
    pub supports_downgrade: bool,
}

#[derive(Clone, Debug)]
pub struct CommandPlan {
    pub executable: String,
    pub args: Vec<String>,
    pub requires_root: bool,
}

#[derive(Clone, Debug)]
pub struct InstallSummary {
    pub name: String,
    pub version: String,
    pub download_size_mb: f64,
    pub install_size_mb: f64,
}

#[derive(Clone, Debug)]
pub struct ConflictInfo {
    pub incoming_pkg: String,
    pub installed_pkg: String,
    pub constraint: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AurResolution {
    pub packages: Vec<(String, String, bool, Option<String>)>,
}

#[async_trait]
pub trait PackageManager: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    async fn search(&self, query: &str) -> Result<Vec<PackageModel>, BackendError>;
    async fn info(&self, package: &str) -> Result<Option<PackageModel>, BackendError>;
    async fn list_installed(&self) -> Result<Vec<PackageModel>, BackendError>;

    // TUI Data Fetching
    async fn get_all_packages(&self) -> Result<(Vec<PackageModel>, usize, usize), BackendError>;

    // CLI Installation Wizard
    async fn get_install_summaries(
        &self,
        packages: &[&str],
    ) -> Result<Vec<InstallSummary>, BackendError>;
    async fn detect_conflicts(&self, packages: &[&str]) -> Result<Vec<ConflictInfo>, BackendError>;

    // AUR Resolution (Returns empty/None if not supported)
    async fn resolve_aur(&self, packages: &[&str]) -> Result<AurResolution, BackendError>;
    async fn build_aur_package(
        &self,
        package: &str,
        verbose: bool,
    ) -> Result<std::path::PathBuf, BackendError>;

    // Transaction Builders
    fn build_install(&self, packages: &[&str]) -> Result<CommandPlan, BackendError>;
    fn build_remove(&self, packages: &[&str]) -> Result<CommandPlan, BackendError>;
    fn build_update(&self) -> Result<CommandPlan, BackendError>;
    fn build_install_local(&self, path: &std::path::Path) -> Result<CommandPlan, BackendError>;
    fn build_upgrade(&self) -> Result<CommandPlan, BackendError>;
}
