use crate::backend::traits::{HistoryEntry, LogEntry, SystemStats, TransactionAction};
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
    fn build_downgrade(&self, _package: &str, _version: &str) -> Result<CommandPlan, BackendError> {
        Err(BackendError::NotSupported("pacman needs interactive downgrade or explicit pacman -U".into()))
    }
    
    fn build_interactive_downgrade(&self, package: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "downgrade".into(),
            args: vec![package.to_string()],
            env: Vec::new(),
            requires_root: true,
        })
    }
    

    async fn owns_file(&self, _path: &str) -> Result<Option<PackageModel>, BackendError> { Ok(None) }
    async fn list_files(&self, _package: &str) -> Result<Vec<String>, BackendError> { Ok(Vec::new()) }
    async fn locate_file(&self, _file: &str) -> Result<Vec<String>, BackendError> { Ok(Vec::new()) }
    fn build_clean(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "pacman".into(), args: vec!["-Scc".into()], env: Vec::new(), requires_root: true })
    }
    fn build_clean_orphans(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "pacman".into(), args: vec!["-Qtdq".into()], env: Vec::new(), requires_root: true })
    }
    fn build_fetch(&self, _packages: &[&str]) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "pacman".into(), args: vec!["-Sw".into()], env: Vec::new(), requires_root: true })
    }
    fn build_mark_explicit(&self, _package: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "pacman".into(), args: vec!["-D".into(), "--asexplicit".into()], env: Vec::new(), requires_root: true })
    }
    fn build_mark_dependency(&self, _package: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan { executable: "pacman".into(), args: vec!["-D".into(), "--asdeps".into()], env: Vec::new(), requires_root: true })
    }

    async fn list_group_or_meta(&self, _name: &str) -> Result<Vec<PackageModel>, BackendError> {
        // Simple mock
        Ok(Vec::new())
    }
    
    async fn get_history(&self, _limit: usize) -> Result<Vec<HistoryEntry>, BackendError> {
        let output = tokio::process::Command::new("tail").args(["-n", &(_limit * 10).to_string(), "/var/log/pacman.log"]).output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();
        for line in stdout.lines().rev() {
            if line.contains("[ALPM]") && (line.contains("installed") || line.contains("upgraded") || line.contains("removed")) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 6 {
                    let ts = format!("{} {}", parts[0].trim_start_matches('['), parts[1].trim_end_matches(']'));
                    let action = parts[3].to_string();
                    let pkg = parts[4].to_string();
                    let ver = parts[5].to_string();
                    entries.push(HistoryEntry {
                        timestamp: ts,
                        action,
                        package: pkg,
                        version: ver,
                    });
                    if entries.len() >= _limit { break; }
                }
            }
        }
        Ok(entries)
    }
    
    async fn get_log(&self, lines: usize) -> Result<Vec<LogEntry>, BackendError> {
        let output = tokio::process::Command::new("tail").args(["-n", &lines.to_string(), "/var/log/pacman.log"]).output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();
        for line in stdout.lines() {
            if let Some(idx) = line.find(']') {
                let ts = line[1..idx].to_string();
                let msg = line[idx+1..].trim().to_string();
                entries.push(LogEntry { timestamp: ts, message: msg });
            }
        }
        Ok(entries)
    }
    
    async fn get_stats(&self) -> Result<SystemStats, BackendError> {
        let exp = tokio::process::Command::new("pacman").arg("-Qeq").output().await.unwrap();
        let dep = tokio::process::Command::new("pacman").arg("-Qdq").output().await.unwrap();
        let e_c = String::from_utf8_lossy(&exp.stdout).lines().count();
        let d_c = String::from_utf8_lossy(&dep.stdout).lines().count();
        Ok(SystemStats { explicit: e_c, dependencies: d_c, total: e_c + d_c })
    }
    
    async fn get_orphan_count(&self) -> Result<usize, BackendError> {
        let out = tokio::process::Command::new("pacman").arg("-Qtdq").output().await.unwrap();
        Ok(String::from_utf8_lossy(&out.stdout).lines().count())
    }

    fn name(&self) -> &'static str {
        "pacman"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_external: true,
            supports_downgrade: false,
            supports_interactive_downgrade: true,
            supports_groups: true,

        }
    }

    async fn search(&self, query: &str) -> Result<Vec<PackageModel>, BackendError> {
        let mut results = Vec::new();
        let mut installed_pkgs = std::collections::HashSet::new();

        {
            let alpm_handle = crate::core::alpm_init::init_alpm()
                .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
            let local_db = alpm_handle.localdb();
            
            for pkg in local_db.pkgs() {
                installed_pkgs.insert(pkg.name().to_string());
            }

            for db in alpm_handle.syncdbs() {
                for pkg in db.pkgs() {
                    if pkg.name().to_lowercase().contains(&query.to_lowercase()) {
                        results.push(PackageModel {
                            name: pkg.name().to_string(),
                            version: pkg.version().to_string(),
                            repo: db.name().to_string(),
                            desc: pkg.desc().map(|s| s.to_string()),
                            is_installed: local_db.pkg(pkg.name()).is_ok(),
                            is_upgradable: false,
                            size_mb: pkg.isize() as f64 / 1_048_576.0,
                        });
                    }
                }
            }
        }

        let config = crate::config::load_config();
        if !config.general.repo_only {
            if let Ok(aur_res) = crate::core::aur::search(query).await {
                for pkg in aur_res.results {
                    results.push(PackageModel {
                        name: pkg.name.clone(),
                        version: pkg.version,
                        repo: "aur".to_string(),
                        desc: pkg.description.map(|s| s.to_string()),
                        is_installed: installed_pkgs.contains(&pkg.name),
                        is_upgradable: false,
                        size_mb: 0.0,
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
                    desc: pkg.desc().map(|s| s.to_string()),
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
            env: Vec::new(), requires_root: true,
        })
    }

    fn build_remove(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-Rs".to_string(), "--noconfirm".to_string()]
                .into_iter()
                .chain(packages.iter().map(|s| s.to_string()))
                .collect(),
            env: Vec::new(), requires_root: true,
        })
    }

    fn build_update(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-Sy".to_string(), "--noconfirm".to_string()],
            env: Vec::new(), requires_root: true,
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
            env: Vec::new(), requires_root: true,
        })
    }
    fn build_upgrade(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-Su".to_string(), "--noconfirm".to_string()],
            env: Vec::new(), requires_root: true,
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
                action: TransactionAction::Install,
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

    async fn resolve_external(
        &self,
        packages: &[&str],
    ) -> Result<crate::backend::traits::ExternalResolution, BackendError> {
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
        Ok(crate::backend::traits::ExternalResolution { packages: resolved })
    }

    async fn build_external_package(
        &self,
        package: &str,
        verbose: bool,
    ) -> Result<std::path::PathBuf, BackendError> {
        crate::core::aur::build(package, verbose)
            .await
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))
    }
}
