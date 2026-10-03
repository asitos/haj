pub mod parser;
use crate::backend::traits::{
    BackendError, Capabilities, CommandPlan, ConflictInfo, ExternalResolution, HistoryEntry,
    InstallSummary, LogEntry, PackageManager, SystemStats,
};
use crate::core::package::PackageModel;
use async_trait::async_trait;
use std::process::Stdio;

pub fn handle_apt_error(code: i32, msg: String) -> BackendError {
    if msg.contains("lock-frontend") || msg.contains("/var/lib/apt/lists/lock") || msg.contains("/var/lib/dpkg/lock") || msg.contains("Could not get lock") {
        BackendError::ExecutionError(code, "Package manager is locked by another process".to_string())
    } else {
        BackendError::ExecutionError(code, msg)
    }
}

use tokio::process::Command;

pub struct AptBackend;

#[async_trait]
impl PackageManager for AptBackend {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_external: false,
            supports_downgrade: true,
            supports_interactive_downgrade: false,
            supports_groups: false,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<PackageModel>, BackendError> {
        let output = Command::new("apt-cache")
            .arg("search")
            .arg(query)
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;
            
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parser::parse_apt_cache_search(&stdout))
    }

    async fn info(&self, package: &str) -> Result<Option<PackageModel>, BackendError> {
        let output = Command::new("apt-cache")
            .arg("show")
            .arg(package)
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parser::parse_apt_cache_show(&stdout, package))
    }

    async fn list_installed(&self) -> Result<Vec<PackageModel>, BackendError> {
        let output = Command::new("dpkg-query")
            .arg("-W")
            .arg("-f=${binary:Package}\t${Version}\t${Status}\n")
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;

        if !output.status.success() {
            return Err(crate::backend::apt::handle_apt_error(output.status.code().unwrap_or(1), String::from_utf8_lossy(&output.stderr).to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parser::parse_dpkg_query_list(&stdout))
    }

    async fn get_all_packages(&self) -> Result<(Vec<PackageModel>, usize, usize), BackendError> {
        let installed = self.list_installed().await?;
        let count = installed.len();
        Ok((installed, count, 0))
    }

    async fn get_install_summaries(
        &self,
        packages: &[&str],
    ) -> Result<Vec<InstallSummary>, BackendError> {
        let mut cmd = Command::new("apt-get");
        cmd.arg("install").arg("-s").arg("-V");
        for pkg in packages {
            cmd.arg(pkg);
        }
        let output = cmd.output().await.map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;
        let _stdout = String::from_utf8_lossy(&output.stdout);
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parser::parse_apt_get_install_dry_run(&stdout))
    }

    async fn detect_conflicts(
        &self,
        _packages: &[&str],
    ) -> Result<Vec<ConflictInfo>, BackendError> {
        Ok(Vec::new()) // Mock for now
    }

    async fn resolve_external(
        &self,
        _packages: &[&str],
    ) -> Result<ExternalResolution, BackendError> {
        Ok(ExternalResolution { packages: Vec::new() })
    }

    async fn build_external_package(
        &self,
        _package: &str,
        _verbose: bool,
    ) -> Result<std::path::PathBuf, BackendError> {
        Err(BackendError::ExecutionError(1, "AUR not supported by APT".to_string()))
    }

    fn build_install(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        let mut args = vec!["install".to_string(), "-y".to_string()];
        args.extend(packages.iter().map(|s| s.to_string()));
        Ok(CommandPlan { executable: "apt-get".to_string(), args, env: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())], requires_root: true })
    }

    fn build_remove(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        let mut args = vec!["remove".to_string(), "-y".to_string()];
        args.extend(packages.iter().map(|s| s.to_string()));
        Ok(CommandPlan { executable: "apt-get".to_string(), args, env: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())], requires_root: true })
    }

    fn build_update(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "apt-get".to_string(), args: vec!["update".to_string()], env: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())], requires_root: true })
    }

    fn build_upgrade(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "apt-get".to_string(), args: vec!["upgrade".to_string(), "-y".to_string()], env: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())], requires_root: true })
    }

    fn build_install_local(&self, path: &std::path::Path) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "apt-get".to_string(), args: vec!["install".to_string(), "-y".to_string(), path.to_string_lossy().to_string()], env: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())], requires_root: true })
    }

    fn build_fetch(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        let mut args = vec!["download".to_string()];
        args.extend(packages.iter().map(|s| s.to_string()));
        Ok(CommandPlan { executable: "apt-get".to_string(), args, env: Vec::new(), requires_root: false })
    }

    fn build_clean(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "apt-get".to_string(), args: vec!["clean".to_string()], env: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())], requires_root: true })
    }

    fn build_clean_orphans(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "apt-get".to_string(), args: vec!["autoremove".to_string(), "-y".to_string()], env: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())], requires_root: true })
    }

    fn build_mark_explicit(&self, package: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "apt-mark".to_string(), args: vec!["manual".to_string(), package.to_string()], env: Vec::new(), requires_root: true })
    }

    fn build_mark_dependency(&self, package: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "apt-mark".to_string(), args: vec!["auto".to_string(), package.to_string()], env: Vec::new(), requires_root: true })
    }

    fn build_downgrade(&self, package: &str, version: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "apt-get".to_string(),
            args: vec!["install".to_string(), "-y".to_string(), format!("{}={}", package, version)],
            env: Vec::new(), requires_root: true,
        })
    }

    fn build_interactive_downgrade(&self, _package: &str) -> Result<CommandPlan, BackendError> {
        Err(BackendError::NotSupported("apt does not support interactive downgrades".into()))
    }

    async fn get_orphan_count(&self) -> Result<usize, BackendError> {
        let output = Command::new("apt-get").arg("-s").arg("autoremove").output().await.map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut count = 0;
        let mut in_rem = false;
        for line in stdout.lines() {
            if line.starts_with("The following packages will be REMOVED:") {
                in_rem = true;
            } else if line.starts_with("0 upgraded") {
                in_rem = false;
            } else if in_rem && line.starts_with("  ") {
                count += line.trim().split_whitespace().count();
            }
        }
        Ok(count)
    }

    async fn owns_file(&self, path: &str) -> Result<Option<PackageModel>, BackendError> {
        let output = Command::new("dpkg").arg("-S").arg(path).output().await.map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(pkg) = stdout.split(':').next() {
                return self.info(pkg).await;
            }
        }
        Ok(None)
    }

    async fn list_files(&self, package: &str) -> Result<Vec<String>, BackendError> {
        let output = Command::new("dpkg").arg("-L").arg(package).output().await.map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    async fn locate_file(&self, query: &str) -> Result<Vec<String>, BackendError> {
        let output = Command::new("apt-file").arg("search").arg(query).output().await.map_err(|_| BackendError::NotSupported("apt-file is not installed".into()))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    async fn list_group_or_meta(&self, _name: &str) -> Result<Vec<PackageModel>, BackendError> {
        Err(BackendError::NotSupported("Package groups not supported by apt".into()))
    }

    async fn get_history(&self, _limit: usize) -> Result<Vec<HistoryEntry>, BackendError> {
        // Implement parsing of /var/log/apt/history.log
        Ok(Vec::new())
    }

    async fn get_log(&self, lines: usize) -> Result<Vec<LogEntry>, BackendError> {
        let output = Command::new("tail").args(["-n", &lines.to_string(), "/var/log/apt/term.log"]).output().await.map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| LogEntry { timestamp: "".into(), message: s.to_string() }).collect())
    }

    async fn get_stats(&self) -> Result<SystemStats, BackendError> {
        let explicit = Command::new("apt-mark").arg("showmanual").output().await.map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;
        let auto = Command::new("apt-mark").arg("showauto").output().await.map_err(|e| crate::backend::apt::handle_apt_error(1, e.to_string()))?;
        let explicit_count = String::from_utf8_lossy(&explicit.stdout).lines().count();
        let auto_count = String::from_utf8_lossy(&auto.stdout).lines().count();
        Ok(SystemStats {
            explicit: explicit_count,
            dependencies: auto_count,
            total: explicit_count + auto_count,
        })
    }
}

impl AptBackend {
    pub fn new() -> Result<Self, BackendError> {
        Ok(Self)
    }
}
