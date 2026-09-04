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
            Self::ExecutionError(code, stderr) => write!(f, "Execution failed ({}): {}", code, stderr),
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

#[async_trait]
pub trait PackageManager: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    async fn search(&self, query: &str) -> Result<Vec<PackageModel>, BackendError>;
    async fn info(&self, package: &str) -> Result<Option<PackageModel>, BackendError>;
    async fn list_installed(&self) -> Result<Vec<PackageModel>, BackendError>;

    fn build_install(&self, packages: &[&str]) -> Result<CommandPlan, BackendError>;
    fn build_remove(&self, packages: &[&str]) -> Result<CommandPlan, BackendError>;
    fn build_update(&self) -> Result<CommandPlan, BackendError>;
    fn build_upgrade(&self) -> Result<CommandPlan, BackendError>;
}
