use crate::core::package::PackageModel;
use async_trait::async_trait;

#[derive(Debug)]
pub enum BackendError {
    NotFound(String),
    NetworkError(String),
    ExecutionError(i32, String),
    ParseError(String),
    NotSupported(String),
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
            Self::NotSupported(err) => write!(f, "Not supported: {}", err),
        }
    }
}

impl std::error::Error for BackendError {}

#[derive(Clone, Debug)]
pub struct Capabilities {
    pub supports_external: bool,
    pub supports_downgrade: bool,
    pub supports_interactive_downgrade: bool,
    pub supports_groups: bool,
}

#[derive(Clone, Debug)]
pub struct CommandPlan {
    pub executable: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub requires_root: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TransactionAction {
    Install,
    Upgrade,
    Remove,
    Downgrade,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct InstallSummary {
    pub action: TransactionAction,
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
pub struct ExternalResolution {
    pub packages: Vec<(String, String, bool, Option<String>)>,
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub action: String,
    pub package: String,
    pub version: String,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct SystemStats {
    pub explicit: usize,
    pub dependencies: usize,
    pub total: usize,
}

#[async_trait]
pub trait PackageManager: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    async fn search(&self, query: &str) -> Result<Vec<PackageModel>, BackendError>;
    async fn info(&self, package: &str) -> Result<Option<PackageModel>, BackendError>;
    async fn list_installed(&self) -> Result<Vec<PackageModel>, BackendError>;
    async fn get_all_packages(&self) -> Result<(Vec<PackageModel>, usize, usize), BackendError>;

    async fn get_install_summaries(&self, packages: &[&str]) -> Result<Vec<InstallSummary>, BackendError>;
    async fn detect_conflicts(&self, packages: &[&str]) -> Result<Vec<ConflictInfo>, BackendError>;

    async fn resolve_external(&self, packages: &[&str]) -> Result<ExternalResolution, BackendError>;
    async fn build_external_package(&self, package: &str, verbose: bool) -> Result<std::path::PathBuf, BackendError>;

    fn build_install(&self, packages: &[&str]) -> Result<CommandPlan, BackendError>;
    fn build_remove(&self, packages: &[&str]) -> Result<CommandPlan, BackendError>;
    fn build_update(&self) -> Result<CommandPlan, BackendError>;
    fn build_upgrade(&self) -> Result<CommandPlan, BackendError>;
    fn build_install_local(&self, path: &std::path::Path) -> Result<CommandPlan, BackendError>;
    
    fn build_downgrade(&self, package: &str, version: &str) -> Result<CommandPlan, BackendError>;
    fn build_interactive_downgrade(&self, package: &str) -> Result<CommandPlan, BackendError>;
    
    async fn list_group_or_meta(&self, name: &str) -> Result<Vec<PackageModel>, BackendError>;
    async fn get_history(&self, limit: usize) -> Result<Vec<HistoryEntry>, BackendError>;
    async fn get_log(&self, lines: usize) -> Result<Vec<LogEntry>, BackendError>;
    async fn get_stats(&self) -> Result<SystemStats, BackendError>;
    async fn get_orphan_count(&self) -> Result<usize, BackendError>;

    async fn owns_file(&self, path: &str) -> Result<Option<PackageModel>, BackendError>;
    async fn list_files(&self, package: &str) -> Result<Vec<String>, BackendError>;
    async fn locate_file(&self, file: &str) -> Result<Vec<String>, BackendError>;

    fn build_clean(&self) -> Result<CommandPlan, BackendError>;
    fn build_clean_orphans(&self) -> Result<CommandPlan, BackendError>;
    fn build_fetch(&self, packages: &[&str]) -> Result<CommandPlan, BackendError>;
    fn build_mark_explicit(&self, package: &str) -> Result<CommandPlan, BackendError>;
    fn build_mark_dependency(&self, package: &str) -> Result<CommandPlan, BackendError>;
}
