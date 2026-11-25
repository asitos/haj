use crate::backend::traits::{
    BackendError, Capabilities, CommandPlan, ConflictInfo, ExternalResolution, HistoryEntry,
    InstallSummary, LogEntry, PackageDetails, PackageManager, SystemStats,
};
use crate::core::package::PackageModel;
use async_trait::async_trait;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[cfg(feature = "pacman")]
mod alpm_ops;
#[cfg(feature = "pacman")]
use alpm_ops as ops;

#[cfg(not(feature = "pacman"))]
mod cli_ops;
#[cfg(not(feature = "pacman"))]
use cli_ops as ops;

fn parse_pacman_history_lines<I>(lines: I, limit: usize) -> Vec<HistoryEntry>
where
    I: IntoIterator<Item = String>,
{
    let mut matched = Vec::new();
    for line in lines {
        let Some((raw_ts, rest)) = line.split_once(" [ALPM] ") else {
            continue;
        };
        let ts = raw_ts.trim_matches(|c| c == '[' || c == ']').to_string();
        let rest = rest.trim();

        let parsed = if let Some(s) = rest.strip_prefix("installed ") {
            Some(("installed", s))
        } else if let Some(s) = rest.strip_prefix("upgraded ") {
            Some(("upgraded", s))
        } else if let Some(s) = rest.strip_prefix("removed ") {
            Some(("removed", s))
        } else {
            rest.strip_prefix("downgraded ").map(|s| ("downgraded", s))
        };

        if let Some((action, body)) = parsed
            && let Some((pkg, ver)) = body.split_once(' ')
        {
            matched.push(HistoryEntry {
                timestamp: ts,
                action: action.to_string(),
                package: pkg.to_string(),
                version: ver.trim().to_string(),
            });
        }
    }

    let start = matched.len().saturating_sub(limit);
    let mut entries = matched[start..].to_vec();
    entries.reverse();
    entries
}

pub struct PacmanBackend {}

impl PacmanBackend {
    pub fn new() -> Result<Self, BackendError> {
        Ok(Self {})
    }
}

fn has_paccache() -> bool {
    std::process::Command::new("paccache")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[async_trait]
impl PackageManager for PacmanBackend {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn lock_file_path(&self) -> Option<&'static str> {
        Some("/var/lib/pacman/db.lck")
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_external: true,
            supports_downgrade: true,
            supports_interactive_downgrade: true,
            supports_groups: true,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<PackageModel>, BackendError> {
        let config = crate::config::load_config();
        self.search_filtered(query, config.general.repo_only, config.general.aur_only)
            .await
    }

    async fn search_filtered(
        &self,
        query: &str,
        repo_only: bool,
        aur_only: bool,
    ) -> Result<Vec<PackageModel>, BackendError> {
        let query_lower = query.to_lowercase();
        let (mut results, installed_pkgs) =
            ops::search_native_and_installed(&query_lower, !aur_only || repo_only)?;

        if !repo_only {
            match crate::core::aur::search(query).await {
                Ok(aur_res) => {
                    for pkg in aur_res.results {
                        if pkg.name.to_lowercase().contains(&query_lower) {
                            results.push(PackageModel {
                                name: pkg.name.clone(),
                                version: pkg.version,
                                repo: "aur".to_string(),
                                desc: pkg.description,
                                is_installed: installed_pkgs.contains(&pkg.name),
                                is_upgradable: false,
                                size_mb: 0.0,
                            });
                        }
                    }
                }
                Err(e) if aur_only => {
                    return Err(BackendError::NetworkError(e.to_string()));
                }
                Err(_) => {}
            }
        }

        Ok(results)
    }

    async fn info(&self, package: &str) -> Result<Option<PackageModel>, BackendError> {
        Ok(self.details(package).await?.map(|d| d.package))
    }

    async fn details(&self, package: &str) -> Result<Option<PackageDetails>, BackendError> {
        if let Some(details) = ops::native_or_local_details(package)? {
            return Ok(Some(details));
        }

        let config = crate::config::load_config();
        if !config.general.repo_only
            && let Ok(res) = crate::core::aur::info(&[package]).await
            && let Some(pkg) = res.results.into_iter().find(|p| p.name == package)
        {
            return Ok(Some(PackageDetails {
                package: PackageModel {
                    name: pkg.name,
                    version: pkg.version,
                    repo: "aur".to_string(),
                    desc: pkg.description,
                    is_installed: false,
                    is_upgradable: false,
                    size_mb: 0.0,
                },
                install_reason: None,
                depends: pkg.depends,
                required_by: Vec::new(),
                opt_depends: pkg.opt_depends,
                votes: Some(pkg.num_votes),
            }));
        }

        Ok(None)
    }

    async fn list_installed(&self) -> Result<Vec<PackageModel>, BackendError> {
        self.list_installed_filtered(false, false, false).await
    }

    async fn list_installed_filtered(
        &self,
        explicit: bool,
        deps: bool,
        foreign: bool,
    ) -> Result<Vec<PackageModel>, BackendError> {
        ops::list_installed_filtered(explicit, deps, foreign)
    }

    async fn get_all_packages(&self) -> Result<(Vec<PackageModel>, usize, usize), BackendError> {
        ops::get_all_packages()
    }

    async fn get_install_summaries(
        &self,
        packages: &[&str],
    ) -> Result<Vec<InstallSummary>, BackendError> {
        ops::get_install_summaries(packages)
    }

    async fn detect_conflicts(&self, packages: &[&str]) -> Result<Vec<ConflictInfo>, BackendError> {
        ops::detect_conflicts(packages)
    }

    async fn resolve_external(
        &self,
        packages: &[&str],
    ) -> Result<ExternalResolution, BackendError> {
        let res = crate::core::aur::info(packages)
            .await
            .map_err(|e| BackendError::NetworkError(e.to_string()))?;

        let local_versions = ops::local_versions(packages);
        let mut by_name: HashMap<String, crate::core::aur::AurPackage> = res
            .results
            .into_iter()
            .map(|p| (p.name.clone(), p))
            .collect();

        let mut resolved = Vec::new();
        for &pkg in packages {
            if let Some(r) = by_name.remove(pkg) {
                let local_ver = local_versions.get(pkg).cloned();
                let is_update = local_ver
                    .as_ref()
                    .is_some_and(|lv| crate::core::aur::is_newer(&r.version, lv));
                resolved.push((r.name, r.version, is_update, local_ver));
            } else {
                return Err(BackendError::NotFound(pkg.to_string()));
            }
        }
        Ok(ExternalResolution { packages: resolved })
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

    fn build_install(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-S".to_string(), "--noconfirm".to_string()]
                .into_iter()
                .chain(packages.iter().map(|s| s.to_string()))
                .collect(),
            env: Vec::new(),
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
            env: Vec::new(),
            requires_root: true,
        })
    }

    fn build_update(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-Sy".to_string(), "--noconfirm".to_string()],
            env: Vec::new(),
            requires_root: true,
        })
    }

    fn build_upgrade(&self) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec!["-Su".to_string(), "--noconfirm".to_string()],
            env: Vec::new(),
            requires_root: true,
        })
    }

    fn build_install_local(&self, path: &Path) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".to_string(),
            args: vec![
                "-U".to_string(),
                path.to_string_lossy().to_string(),
                "--noconfirm".to_string(),
            ],
            env: Vec::new(),
            requires_root: true,
        })
    }

    fn build_downgrade(&self, package: &str, version: &str) -> Result<CommandPlan, BackendError> {
        let Some(archive_path) = crate::core::downgrade::find_cached_version(package, version)
        else {
            return Err(BackendError::NotFound(format!(
                "cached archive for '{package}' version '{version}' not found"
            )));
        };
        self.build_install_local(&archive_path)
    }

    fn build_interactive_downgrade(&self, package: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "downgrade".into(),
            args: vec![package.to_string()],
            env: Vec::new(),
            requires_root: true,
        })
    }

    async fn list_group_or_meta(&self, name: &str) -> Result<Vec<PackageModel>, BackendError> {
        ops::list_group_or_meta(name)
    }

    async fn get_history(&self, limit: usize) -> Result<Vec<HistoryEntry>, BackendError> {
        let Ok(file) = std::fs::File::open("/var/log/pacman.log") else {
            return Ok(Vec::new());
        };
        Ok(parse_pacman_history_lines(
            BufReader::new(file).lines().map_while(Result::ok),
            limit,
        ))
    }

    async fn get_log(&self, lines: usize) -> Result<Vec<LogEntry>, BackendError> {
        let Ok(file) = std::fs::File::open("/var/log/pacman.log") else {
            return Ok(Vec::new());
        };
        let reader = BufReader::new(file);
        let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
        let start = all_lines.len().saturating_sub(lines);

        let entries = all_lines[start..]
            .iter()
            .filter_map(|line| {
                let idx = line.find(']')?;
                if !line.starts_with('[') {
                    return None;
                }
                let ts = line[1..idx].to_string();
                let msg = line[idx + 1..].trim().to_string();
                Some(LogEntry {
                    timestamp: ts,
                    message: msg,
                })
            })
            .collect();

        Ok(entries)
    }

    async fn get_stats(&self) -> Result<SystemStats, BackendError> {
        ops::get_stats()
    }

    async fn get_orphan_count(&self) -> Result<usize, BackendError> {
        ops::get_orphan_count()
    }

    async fn list_orphans(&self) -> Result<Vec<PackageModel>, BackendError> {
        ops::list_orphans()
    }

    async fn owns_file(&self, path: &str) -> Result<Option<PackageModel>, BackendError> {
        ops::owns_file(path)
    }

    async fn list_files(&self, package: &str) -> Result<Vec<String>, BackendError> {
        ops::list_files(package)
    }

    async fn locate_file(&self, file: &str) -> Result<Vec<String>, BackendError> {
        let output = tokio::process::Command::new("pacman")
            .arg("-F")
            .arg(file)
            .output()
            .await
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Ok(stdout
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|s| s.to_string())
                .collect());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            return Err(BackendError::ExecutionError(
                output.status.code().unwrap_or(1),
                stderr,
            ));
        }

        Ok(Vec::new())
    }

    fn build_clean(&self) -> Result<CommandPlan, BackendError> {
        self.build_clean_keep(3)
    }

    fn build_clean_keep(&self, keep: usize) -> Result<CommandPlan, BackendError> {
        if has_paccache() {
            Ok(CommandPlan {
                executable: "paccache".into(),
                args: vec!["-r".into(), "-k".into(), keep.to_string()],
                env: Vec::new(),
                requires_root: true,
            })
        } else {
            let flag = if keep == 0 { "-Scc" } else { "-Sc" };
            Ok(CommandPlan {
                executable: "pacman".into(),
                args: vec![flag.into(), "--noconfirm".into()],
                env: Vec::new(),
                requires_root: true,
            })
        }
    }

    fn build_clean_orphans(&self) -> Result<CommandPlan, BackendError> {
        let orphans = ops::list_orphans()?;
        if orphans.is_empty() {
            return Err(BackendError::NotFound(
                "no orphaned packages found".to_string(),
            ));
        }

        let mut args = vec!["-Rns".to_string(), "--noconfirm".to_string()];
        args.extend(orphans.into_iter().map(|p| p.name));

        Ok(CommandPlan {
            executable: "pacman".into(),
            args,
            env: Vec::new(),
            requires_root: true,
        })
    }

    fn build_fetch(&self, packages: &[&str]) -> Result<CommandPlan, BackendError> {
        let mut args = vec!["-Sw".to_string(), "--noconfirm".to_string()];
        args.extend(packages.iter().map(|s| s.to_string()));
        Ok(CommandPlan {
            executable: "pacman".into(),
            args,
            env: Vec::new(),
            requires_root: true,
        })
    }

    fn build_mark_explicit(&self, package: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".into(),
            args: vec!["-D".into(), "--asexplicit".into(), package.to_string()],
            env: Vec::new(),
            requires_root: true,
        })
    }

    fn build_mark_dependency(&self, package: &str) -> Result<CommandPlan, BackendError> {
        Ok(CommandPlan {
            executable: "pacman".into(),
            args: vec!["-D".into(), "--asdeps".into(), package.to_string()],
            env: Vec::new(),
            requires_root: true,
        })
    }

    async fn toggle_mark(
        &self,
        package: &str,
    ) -> Result<(CommandPlan, &'static str), BackendError> {
        if ops::is_explicitly_installed(package)? {
            Ok((self.build_mark_dependency(package)?, "dependency"))
        } else {
            Ok((self.build_mark_explicit(package)?, "explicit"))
        }
    }

    async fn check_native_updates(&self) -> Result<Vec<(String, String, String)>, BackendError> {
        let output = tokio::process::Command::new("pacman")
            .arg("-Qu")
            .output()
            .await
            .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let updates = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    Some((
                        parts[0].to_string(),
                        parts[1].to_string(),
                        parts[3].to_string(),
                    ))
                } else {
                    None
                }
            })
            .collect();
        Ok(updates)
    }

    async fn check_external_updates(&self) -> Result<Vec<(String, String, String)>, BackendError> {
        let foreign_pkgs = ops::foreign_versions()?;

        let mut updates = Vec::new();
        for chunk in foreign_pkgs.chunks(50) {
            let names: Vec<&str> = chunk.iter().map(|(n, _)| n.as_str()).collect();
            if let Ok(res) = crate::core::aur::info(&names).await {
                for pkg in res.results {
                    if let Some((_, local_ver)) = chunk.iter().find(|(n, _)| n == &pkg.name)
                        && crate::core::aur::is_newer(&pkg.version, local_ver)
                    {
                        updates.push((pkg.name, local_ver.clone(), pkg.version));
                    }
                }
            }
        }
        Ok(updates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_iso_pacman_history_lines() {
        let lines = vec![
            "[2026-11-25T18:34:28+0530] [ALPM] transaction started".to_string(),
            "[2026-11-25T18:34:28+0530] [ALPM] removed beeper-bin (1:4.3.308-1)".to_string(),
            "[2026-11-25T18:37:00+0530] [ALPM] upgraded foo (1.0-1 -> 2.0-1)".to_string(),
            "[2026-11-25T18:53:34+0530] [ALPM] installed zapfast-bin (0.21.3-1)".to_string(),
        ];
        let parsed = parse_pacman_history_lines(lines, 2);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].action, "installed");
        assert_eq!(parsed[0].package, "zapfast-bin");
        assert_eq!(parsed[0].version, "(0.21.3-1)");
        assert_eq!(parsed[1].action, "upgraded");
        assert_eq!(parsed[1].package, "foo");
    }
}
