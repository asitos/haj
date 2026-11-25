use crate::backend::traits::{
    BackendError, ConflictInfo, InstallSummary, PackageDetails, SystemStats, TransactionAction,
};
use crate::core::package::PackageModel;
use std::collections::{HashMap, HashSet};
use std::path::Path;

fn init_alpm_backend() -> Result<alpm::Alpm, BackendError> {
    crate::core::alpm_init::init_alpm().map_err(|e| BackendError::ExecutionError(1, e.to_string()))
}

fn is_orphan(pkg: &alpm::Package) -> bool {
    pkg.reason() == alpm::PackageReason::Depend
        && pkg.required_by().is_empty()
        && pkg.optional_for().is_empty()
}

fn collect_orphans(alpm_handle: &alpm::Alpm) -> Vec<PackageModel> {
    alpm_handle
        .localdb()
        .pkgs()
        .iter()
        .filter(|pkg| is_orphan(pkg))
        .map(|pkg| PackageModel {
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            repo: "local".to_string(),
            is_installed: true,
            is_upgradable: false,
            size_mb: pkg.isize() as f64 / 1_048_576.0,
            desc: pkg.desc().map(|s| s.to_string()),
        })
        .collect()
}

pub fn foreign_versions() -> Result<Vec<(String, String)>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    Ok(alpm_handle
        .localdb()
        .pkgs()
        .iter()
        .filter(|pkg| {
            !alpm_handle
                .syncdbs()
                .iter()
                .any(|db| db.pkg(pkg.name()).is_ok())
        })
        .map(|pkg| (pkg.name().to_string(), pkg.version().to_string()))
        .collect())
}

pub fn search_native_and_installed(
    query_lower: &str,
    include_sync: bool,
) -> Result<(Vec<PackageModel>, HashSet<String>), BackendError> {
    let mut results = Vec::new();
    let mut installed_pkgs = HashSet::new();

    let alpm_handle = init_alpm_backend()?;
    let local_db = alpm_handle.localdb();

    for pkg in local_db.pkgs() {
        installed_pkgs.insert(pkg.name().to_string());
    }

    if include_sync {
        for db in alpm_handle.syncdbs() {
            for pkg in db.pkgs() {
                if pkg.name().to_lowercase().contains(query_lower) {
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

    Ok((results, installed_pkgs))
}

pub fn native_or_local_details(package: &str) -> Result<Option<PackageDetails>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let local_db = alpm_handle.localdb();

    for db in alpm_handle.syncdbs() {
        if let Ok(pkg) = db.pkg(package) {
            let local_pkg = local_db.pkg(package).ok();
            let install_reason = local_pkg.as_ref().map(|lp| match lp.reason() {
                alpm::PackageReason::Explicit => "explicitly installed".to_string(),
                alpm::PackageReason::Depend => "installed as a dependency".to_string(),
            });
            let required_by = local_pkg
                .as_ref()
                .map(|lp| lp.required_by().iter().map(|s| s.to_string()).collect())
                .unwrap_or_default();

            return Ok(Some(PackageDetails {
                package: PackageModel {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    repo: db.name().to_string(),
                    desc: pkg.desc().map(|s| s.to_string()),
                    is_installed: local_pkg.is_some(),
                    is_upgradable: false,
                    size_mb: pkg.isize() as f64 / 1_048_576.0,
                },
                install_reason,
                depends: pkg.depends().iter().map(|d| d.name().to_string()).collect(),
                required_by,
                opt_depends: pkg
                    .optdepends()
                    .iter()
                    .map(|d| d.name().to_string())
                    .collect(),
                votes: None,
            }));
        }
    }

    if let Ok(pkg) = local_db.pkg(package) {
        let reason = match pkg.reason() {
            alpm::PackageReason::Explicit => "explicitly installed".to_string(),
            alpm::PackageReason::Depend => "installed as a dependency".to_string(),
        };
        return Ok(Some(PackageDetails {
            package: PackageModel {
                name: pkg.name().to_string(),
                version: pkg.version().to_string(),
                repo: "local/aur".to_string(),
                desc: pkg.desc().map(|s| s.to_string()),
                is_installed: true,
                is_upgradable: false,
                size_mb: pkg.isize() as f64 / 1_048_576.0,
            },
            install_reason: Some(reason),
            depends: pkg.depends().iter().map(|d| d.name().to_string()).collect(),
            required_by: pkg.required_by().iter().map(|s| s.to_string()).collect(),
            opt_depends: pkg
                .optdepends()
                .iter()
                .map(|d| d.name().to_string())
                .collect(),
            votes: None,
        }));
    }

    Ok(None)
}

pub fn list_installed_filtered(
    explicit: bool,
    deps: bool,
    foreign: bool,
) -> Result<Vec<PackageModel>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let local_db = alpm_handle.localdb();
    let mut results = Vec::new();

    for pkg in local_db.pkgs() {
        let is_explicit = pkg.reason() == alpm::PackageReason::Explicit;
        if explicit && !is_explicit {
            continue;
        }
        if deps && is_explicit {
            continue;
        }

        let found_in_repo = alpm_handle
            .syncdbs()
            .iter()
            .any(|db| db.pkg(pkg.name()).is_ok());
        if foreign && found_in_repo {
            continue;
        }

        results.push(PackageModel {
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            repo: if found_in_repo { "native" } else { "aur" }.to_string(),
            is_installed: true,
            is_upgradable: false,
            size_mb: pkg.isize() as f64 / 1_048_576.0,
            desc: pkg.desc().map(|s| s.to_string()),
        });
    }

    Ok(results)
}

pub fn get_all_packages() -> Result<(Vec<PackageModel>, usize, usize), BackendError> {
    let alpm_handle = init_alpm_backend()?;
    Ok(crate::core::pacman::get_all_packages(&alpm_handle))
}

pub fn get_install_summaries(packages: &[&str]) -> Result<Vec<InstallSummary>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let pkgs = packages.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let summaries = crate::core::get_install_summaries(&alpm_handle, &pkgs)
        .map_err(|e| BackendError::ExecutionError(1, e.to_string()))?;
    Ok(summaries
        .into_iter()
        .map(|s| InstallSummary {
            action: TransactionAction::Install,
            name: s.name,
            version: s.version,
            download_size_mb: s.download_size_mb,
            install_size_mb: s.install_size_mb,
        })
        .collect())
}

pub fn detect_conflicts(packages: &[&str]) -> Result<Vec<ConflictInfo>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let pkgs = packages.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let conflicts = crate::core::conflicts::detect_conflicts(&alpm_handle, &pkgs, &HashMap::new());
    Ok(conflicts
        .into_iter()
        .map(|c| ConflictInfo {
            incoming_pkg: c.incoming_pkg,
            installed_pkg: c.installed_pkg,
            constraint: c.constraint,
        })
        .collect())
}

pub fn local_versions(packages: &[&str]) -> HashMap<String, String> {
    let Ok(alpm_handle) = init_alpm_backend() else {
        return HashMap::new();
    };
    let local_db = alpm_handle.localdb();
    packages
        .iter()
        .filter_map(|&pkg| {
            local_db
                .pkg(pkg)
                .ok()
                .map(|p| (pkg.to_string(), p.version().to_string()))
        })
        .collect()
}

pub fn list_group_or_meta(name: &str) -> Result<Vec<PackageModel>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let local_db = alpm_handle.localdb();
    let mut seen = HashSet::new();
    let mut group_pkgs = Vec::new();

    for db in alpm_handle.syncdbs() {
        for pkg in db.pkgs() {
            if pkg.groups().iter().any(|g| g == name) && seen.insert(pkg.name().to_string()) {
                group_pkgs.push(PackageModel {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    repo: db.name().to_string(),
                    is_installed: local_db.pkg(pkg.name()).is_ok(),
                    is_upgradable: false,
                    size_mb: pkg.isize() as f64 / 1_048_576.0,
                    desc: pkg.desc().map(|s| s.to_string()),
                });
            }
        }
    }

    if group_pkgs.is_empty() {
        for db in alpm_handle.syncdbs() {
            if let Ok(meta) = db.pkg(name)
                && meta.isize() == 0
                && !meta.depends().is_empty()
            {
                for dep in meta.depends() {
                    let dep_name = dep.name();
                    if !seen.insert(dep_name.to_string()) {
                        continue;
                    }
                    for dep_db in alpm_handle.syncdbs() {
                        if let Ok(pkg) = dep_db.pkg(dep_name) {
                            group_pkgs.push(PackageModel {
                                name: pkg.name().to_string(),
                                version: pkg.version().to_string(),
                                repo: dep_db.name().to_string(),
                                is_installed: local_db.pkg(pkg.name()).is_ok(),
                                is_upgradable: false,
                                size_mb: pkg.isize() as f64 / 1_048_576.0,
                                desc: pkg.desc().map(|s| s.to_string()),
                            });
                            break;
                        }
                    }
                }
                break;
            }
        }
    }

    Ok(group_pkgs)
}

pub fn get_stats() -> Result<SystemStats, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let mut explicit = 0usize;
    let mut dependencies = 0usize;
    for pkg in alpm_handle.localdb().pkgs() {
        match pkg.reason() {
            alpm::PackageReason::Explicit => explicit += 1,
            alpm::PackageReason::Depend => dependencies += 1,
        }
    }
    Ok(SystemStats {
        explicit,
        dependencies,
        total: explicit + dependencies,
    })
}

pub fn get_orphan_count() -> Result<usize, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    Ok(alpm_handle
        .localdb()
        .pkgs()
        .iter()
        .filter(|p| is_orphan(p))
        .count())
}

pub fn list_orphans() -> Result<Vec<PackageModel>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    Ok(collect_orphans(&alpm_handle))
}

pub fn owns_file(path: &str) -> Result<Option<PackageModel>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let target = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(path).to_path_buf());
    let clean_target = target.to_string_lossy();
    let clean_rel = clean_target.trim_start_matches('/');
    let raw_rel = path.trim_start_matches('/');

    for pkg in alpm_handle.localdb().pkgs() {
        let files = pkg.files();
        if files.contains(clean_rel).is_some() || files.contains(raw_rel).is_some() {
            return Ok(Some(PackageModel {
                name: pkg.name().to_string(),
                version: pkg.version().to_string(),
                repo: "local".to_string(),
                is_installed: true,
                is_upgradable: false,
                size_mb: pkg.isize() as f64 / 1_048_576.0,
                desc: pkg.desc().map(|s| s.to_string()),
            }));
        }
    }

    Ok(None)
}

pub fn list_files(package: &str) -> Result<Vec<String>, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let pkg = alpm_handle
        .localdb()
        .pkg(package)
        .map_err(|_| BackendError::NotFound(package.to_string()))?;

    Ok(pkg
        .files()
        .files()
        .iter()
        .map(|f| format!("/{}", String::from_utf8_lossy(f.name())))
        .collect())
}

pub fn is_explicitly_installed(package: &str) -> Result<bool, BackendError> {
    let alpm_handle = init_alpm_backend()?;
    let pkg = alpm_handle
        .localdb()
        .pkg(package)
        .map_err(|_| BackendError::NotFound(package.to_string()))?;

    Ok(pkg.reason() == alpm::PackageReason::Explicit)
}
