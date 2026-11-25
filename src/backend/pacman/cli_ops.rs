use crate::backend::traits::{
    BackendError, ConflictInfo, InstallSummary, PackageDetails, SystemStats, TransactionAction,
};
use crate::core::package::PackageModel;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::{Command, Output};

type Stanza = HashMap<String, Vec<String>>;

fn pacman_output(args: &[&str]) -> Result<Output, BackendError> {
    Command::new("pacman")
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .map_err(|e| BackendError::ExecutionError(1, e.to_string()))
}

fn pacman_lines(args: &[&str]) -> Result<Vec<String>, BackendError> {
    let out = pacman_output(args)?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if stderr.is_empty() {
        return Ok(Vec::new());
    }
    Err(BackendError::ExecutionError(
        out.status.code().unwrap_or(1),
        stderr,
    ))
}

fn escape_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn parse_stanzas(raw: &str) -> Vec<Stanza> {
    let mut stanzas = Vec::new();
    let mut current: Stanza = HashMap::new();
    let mut last_key: Option<String> = None;

    for line in raw.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                stanzas.push(std::mem::take(&mut current));
            }
            last_key = None;
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(ref key) = last_key
                && let Some(vals) = current.get_mut(key)
            {
                vals.push(line.trim().to_string());
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            current.insert(key.clone(), vec![val]);
            last_key = Some(key);
        }
    }

    if !current.is_empty() {
        stanzas.push(current);
    }
    stanzas
}

fn field_str(stanza: &Stanza, key: &str) -> Option<String> {
    let vals = stanza.get(key)?;
    let joined = vals.join(" ");
    let trimmed = joined.trim();
    if trimmed.is_empty() || trimmed == "None" {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn field_tokens(stanza: &Stanza, key: &str) -> Vec<String> {
    let Some(vals) = stanza.get(key) else {
        return Vec::new();
    };
    vals.iter()
        .flat_map(|line| line.split_whitespace())
        .filter(|tok| *tok != "None")
        .map(strip_dep_constraint)
        .collect()
}

fn field_optdeps(stanza: &Stanza) -> Vec<String> {
    let Some(vals) = stanza.get("Optional Deps") else {
        return Vec::new();
    };
    vals.iter()
        .filter(|line| line.trim() != "None")
        .filter_map(|line| {
            let name = line.split(':').next()?.trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

fn strip_dep_constraint(dep: &str) -> String {
    dep.split(['<', '>', '=']).next().unwrap_or(dep).to_string()
}

fn parse_size_mb(raw: Option<String>) -> f64 {
    let Some(s) = raw else {
        return 0.0;
    };
    let mut parts = s.split_whitespace();
    let Some(num) = parts.next().and_then(|v| v.parse::<f64>().ok()) else {
        return 0.0;
    };
    match parts.next().unwrap_or("B") {
        "B" => num / 1_048_576.0,
        "KiB" => num / 1024.0,
        "MiB" => num,
        "GiB" => num * 1024.0,
        "TiB" => num * 1_048_576.0,
        _ => 0.0,
    }
}

fn parse_install_reason(stanza: &Stanza) -> Option<String> {
    field_str(stanza, "Install Reason").map(|r| {
        if r.starts_with("Explicitly") {
            "explicitly installed".to_string()
        } else {
            "installed as a dependency".to_string()
        }
    })
}

fn model_from_stanza(stanza: &Stanza, repo: String, is_installed: bool) -> PackageModel {
    PackageModel {
        name: field_str(stanza, "Name").unwrap_or_default(),
        version: field_str(stanza, "Version").unwrap_or_default(),
        repo,
        desc: field_str(stanza, "Description"),
        is_installed,
        is_upgradable: false,
        size_mb: parse_size_mb(field_str(stanza, "Installed Size")),
    }
}

fn fetch_first_stanzas(args: &[&str]) -> HashMap<String, Stanza> {
    let Ok(out) = pacman_output(args) else {
        return HashMap::new();
    };
    let mut by_name = HashMap::new();
    for stanza in parse_stanzas(&String::from_utf8_lossy(&out.stdout)) {
        if let Some(name) = field_str(&stanza, "Name") {
            by_name.entry(name).or_insert(stanza);
        }
    }
    by_name
}

pub fn search_native_and_installed(
    query_lower: &str,
    include_sync: bool,
) -> Result<(Vec<PackageModel>, HashSet<String>), BackendError> {
    let installed_pkgs: HashSet<String> = pacman_lines(&["-Qq"])?.into_iter().collect();
    let mut results = Vec::new();

    if include_sync {
        let pattern = escape_regex(query_lower);
        let out = pacman_output(&["-Ss", &pattern])?;
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut lines = stdout.lines().peekable();
            while let Some(header) = lines.next() {
                if header.starts_with(' ') || header.trim().is_empty() {
                    continue;
                }
                let desc = if lines.peek().is_some_and(|l| l.starts_with(' ')) {
                    lines.next().map(|l| l.trim().to_string())
                } else {
                    None
                };

                let mut parts = header.split_whitespace();
                let Some(repo_name) = parts.next() else {
                    continue;
                };
                let Some((repo, name)) = repo_name.split_once('/') else {
                    continue;
                };
                let version = parts.next().unwrap_or_default();

                if name.to_lowercase().contains(query_lower) {
                    results.push(PackageModel {
                        name: name.to_string(),
                        version: version.to_string(),
                        repo: repo.to_string(),
                        desc,
                        is_installed: installed_pkgs.contains(name),
                        is_upgradable: false,
                        size_mb: 0.0,
                    });
                }
            }
        }
    }

    Ok((results, installed_pkgs))
}

pub fn native_or_local_details(package: &str) -> Result<Option<PackageDetails>, BackendError> {
    let sync_out = pacman_output(&["-Si", package])?;
    let local_out = pacman_output(&["-Qi", package])?;
    let local_stanza = if local_out.status.success() {
        parse_stanzas(&String::from_utf8_lossy(&local_out.stdout))
            .into_iter()
            .next()
    } else {
        None
    };

    if sync_out.status.success()
        && let Some(sync_stanza) = parse_stanzas(&String::from_utf8_lossy(&sync_out.stdout))
            .into_iter()
            .next()
    {
        let repo = field_str(&sync_stanza, "Repository").unwrap_or_else(|| "sync".to_string());
        let install_reason = local_stanza.as_ref().and_then(parse_install_reason);
        let required_by = local_stanza
            .as_ref()
            .map(|s| field_tokens(s, "Required By"))
            .unwrap_or_default();

        return Ok(Some(PackageDetails {
            package: model_from_stanza(&sync_stanza, repo, local_stanza.is_some()),
            install_reason,
            depends: field_tokens(&sync_stanza, "Depends On"),
            required_by,
            opt_depends: field_optdeps(&sync_stanza),
            votes: None,
        }));
    }

    if let Some(local) = local_stanza {
        let install_reason = parse_install_reason(&local);
        return Ok(Some(PackageDetails {
            package: model_from_stanza(&local, "local/aur".to_string(), true),
            install_reason,
            depends: field_tokens(&local, "Depends On"),
            required_by: field_tokens(&local, "Required By"),
            opt_depends: field_optdeps(&local),
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
    let foreign_set: HashSet<String> = pacman_lines(&["-Qmq"])?.into_iter().collect();
    let out = pacman_output(&["-Qi"])?;
    if !out.status.success() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    for stanza in parse_stanzas(&String::from_utf8_lossy(&out.stdout)) {
        let Some(name) = field_str(&stanza, "Name") else {
            continue;
        };
        let is_explicit =
            field_str(&stanza, "Install Reason").is_some_and(|r| r.starts_with("Explicitly"));
        if explicit && !is_explicit {
            continue;
        }
        if deps && is_explicit {
            continue;
        }
        let is_foreign = foreign_set.contains(&name);
        if foreign && !is_foreign {
            continue;
        }

        let repo = if is_foreign { "aur" } else { "native" }.to_string();
        results.push(model_from_stanza(&stanza, repo, true));
    }

    Ok(results)
}

pub fn get_all_packages() -> Result<(Vec<PackageModel>, usize, usize), BackendError> {
    let local_out = pacman_output(&["-Q"])?;
    let mut local_versions = HashMap::new();
    if local_out.status.success() {
        for line in String::from_utf8_lossy(&local_out.stdout).lines() {
            let mut parts = line.split_whitespace();
            if let (Some(name), Some(ver)) = (parts.next(), parts.next()) {
                local_versions.insert(name.to_string(), ver.to_string());
            }
        }
    }
    let local_count = local_versions.len();

    let sl_out = pacman_output(&["-Sl"])?;
    let mut packages = Vec::new();
    let mut seen_names = HashSet::new();
    let mut sync_count = 0usize;

    if sl_out.status.success() {
        for line in String::from_utf8_lossy(&sl_out.stdout).lines() {
            let mut parts = line.split_whitespace();
            let (Some(repo), Some(name), Some(version)) =
                (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            sync_count += 1;
            if !seen_names.insert(name.to_string()) {
                continue;
            }
            let is_installed = local_versions.contains_key(name);
            let is_upgradable = line.contains("[installed: ");
            packages.push(PackageModel {
                name: name.to_string(),
                version: version.to_string(),
                repo: repo.to_string(),
                desc: None,
                is_installed,
                is_upgradable,
                size_mb: 0.0,
            });
        }
    }

    for (name, version) in local_versions {
        if seen_names.insert(name.clone()) {
            packages.push(PackageModel {
                name,
                version,
                repo: "aur".to_string(),
                desc: None,
                is_installed: true,
                is_upgradable: false,
                size_mb: 0.0,
            });
        }
    }

    Ok((packages, local_count, sync_count))
}

pub fn get_install_summaries(packages: &[&str]) -> Result<Vec<InstallSummary>, BackendError> {
    if packages.is_empty() {
        return Ok(Vec::new());
    }

    let mut args = vec!["-Si"];
    args.extend_from_slice(packages);
    let out = pacman_output(&args)?;
    let mut by_name = HashMap::new();
    for stanza in parse_stanzas(&String::from_utf8_lossy(&out.stdout)) {
        if let Some(name) = field_str(&stanza, "Name") {
            by_name.entry(name).or_insert(stanza);
        }
    }

    let mut summaries = Vec::new();
    for &target in packages {
        let lookup = target.split('/').next_back().unwrap_or(target);
        let Some(stanza) = by_name.get(lookup) else {
            return Err(BackendError::ExecutionError(
                1,
                format!("package '{target}' not found in any repository."),
            ));
        };
        summaries.push(InstallSummary {
            action: TransactionAction::Install,
            name: field_str(stanza, "Name").unwrap_or_else(|| lookup.to_string()),
            version: field_str(stanza, "Version").unwrap_or_default(),
            download_size_mb: parse_size_mb(field_str(stanza, "Download Size")),
            install_size_mb: parse_size_mb(field_str(stanza, "Installed Size")),
        });
    }

    Ok(summaries)
}

pub fn detect_conflicts(packages: &[&str]) -> Result<Vec<ConflictInfo>, BackendError> {
    if packages.is_empty() {
        return Ok(Vec::new());
    }
    let installed: HashSet<String> = pacman_lines(&["-Qq"])?.into_iter().collect();
    let mut args = vec!["-Si"];
    args.extend_from_slice(packages);
    let stanzas = fetch_first_stanzas(&args);

    let mut conflicts = Vec::new();
    for &pkg in packages {
        let lookup = pkg.split('/').next_back().unwrap_or(pkg);
        let Some(stanza) = stanzas.get(lookup) else {
            continue;
        };
        for raw_conflict in stanza
            .get("Conflicts With")
            .into_iter()
            .flatten()
            .flat_map(|l| l.split_whitespace())
            .filter(|t| *t != "None")
        {
            let target_name = strip_dep_constraint(raw_conflict);
            if target_name != lookup && installed.contains(&target_name) {
                conflicts.push(ConflictInfo {
                    incoming_pkg: lookup.to_string(),
                    installed_pkg: target_name,
                    constraint: Some(raw_conflict.to_string()),
                });
            }
        }
    }

    Ok(conflicts)
}

pub fn local_versions(_packages: &[&str]) -> HashMap<String, String> {
    let Ok(out) = pacman_output(&["-Q"]) else {
        return HashMap::new();
    };
    if !out.status.success() {
        return HashMap::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        })
        .collect()
}

pub fn list_group_or_meta(name: &str) -> Result<Vec<PackageModel>, BackendError> {
    let installed: HashSet<String> = pacman_lines(&["-Qq"])?.into_iter().collect();
    let mut member_names = pacman_lines(&["-Sgq", name])?;

    if member_names.is_empty() {
        let meta_out = pacman_output(&["-Si", name])?;
        if meta_out.status.success()
            && let Some(meta) = parse_stanzas(&String::from_utf8_lossy(&meta_out.stdout))
                .into_iter()
                .next()
        {
            let size_mb = parse_size_mb(field_str(&meta, "Installed Size"));
            if size_mb == 0.0 {
                member_names = field_tokens(&meta, "Depends On");
            }
        }
    }

    if member_names.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen = HashSet::new();
    let unique_names: Vec<String> = member_names
        .into_iter()
        .filter(|m| seen.insert(m.clone()))
        .collect();

    let mut args = vec!["-Si"];
    args.extend(unique_names.iter().map(String::as_str));
    let stanzas = fetch_first_stanzas(&args);

    Ok(unique_names
        .into_iter()
        .filter_map(|pkg_name| {
            let stanza = stanzas.get(&pkg_name)?;
            let repo = field_str(stanza, "Repository").unwrap_or_else(|| "sync".to_string());
            Some(model_from_stanza(
                stanza,
                repo,
                installed.contains(&pkg_name),
            ))
        })
        .collect())
}

pub fn get_stats() -> Result<SystemStats, BackendError> {
    let explicit = pacman_lines(&["-Qeq"])?.len();
    let dependencies = pacman_lines(&["-Qdq"])?.len();
    Ok(SystemStats {
        explicit,
        dependencies,
        total: explicit + dependencies,
    })
}

pub fn get_orphan_count() -> Result<usize, BackendError> {
    Ok(pacman_lines(&["-Qdtq"])?.len())
}

pub fn list_orphans() -> Result<Vec<PackageModel>, BackendError> {
    let names = pacman_lines(&["-Qdtq"])?;
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["-Qi"];
    args.extend(names.iter().map(String::as_str));
    let out = pacman_output(&args)?;
    if !out.status.success() {
        return Ok(Vec::new());
    }

    Ok(parse_stanzas(&String::from_utf8_lossy(&out.stdout))
        .into_iter()
        .filter_map(|stanza| {
            let _ = field_str(&stanza, "Name")?;
            Some(model_from_stanza(&stanza, "local".to_string(), true))
        })
        .collect())
}

pub fn owns_file(path: &str) -> Result<Option<PackageModel>, BackendError> {
    let target = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(path).to_path_buf());
    let target_str = target.to_string_lossy();

    let mut owner_out = pacman_output(&["-Qoq", &target_str])?;
    if !owner_out.status.success() && target_str != path {
        owner_out = pacman_output(&["-Qoq", path])?;
    }
    if !owner_out.status.success() {
        return Ok(None);
    }

    let Some(owner) = String::from_utf8_lossy(&owner_out.stdout)
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
    else {
        return Ok(None);
    };

    let info_out = pacman_output(&["-Qi", &owner])?;
    if info_out.status.success()
        && let Some(stanza) = parse_stanzas(&String::from_utf8_lossy(&info_out.stdout))
            .into_iter()
            .next()
    {
        return Ok(Some(model_from_stanza(&stanza, "local".to_string(), true)));
    }

    Ok(None)
}

pub fn list_files(package: &str) -> Result<Vec<String>, BackendError> {
    let out = pacman_output(&["-Qlq", package])?;
    if !out.status.success() {
        return Err(BackendError::NotFound(package.to_string()));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect())
}

pub fn is_explicitly_installed(package: &str) -> Result<bool, BackendError> {
    let out = pacman_output(&["-Qi", package])?;
    if !out.status.success() {
        return Err(BackendError::NotFound(package.to_string()));
    }
    let is_explicit = parse_stanzas(&String::from_utf8_lossy(&out.stdout))
        .into_iter()
        .next()
        .and_then(|s| field_str(&s, "Install Reason"))
        .is_some_and(|r| r.starts_with("Explicitly"));
    Ok(is_explicit)
}

pub fn foreign_versions() -> Result<Vec<(String, String)>, BackendError> {
    Ok(pacman_lines(&["-Qm"])?
        .into_iter()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_INFO: &str = "\
Repository      : core
Name            : bash
Version         : 5.3.20-2
Description     : The GNU Bourne Again shell
Depends On      : readline  libreadline.so=8-64  glibc>=2.38  ncurses
Optional Deps   : bash-completion: for tab completion
                  less: for pager support
Required By     : 7zip  base
                  pacman
Conflicts With  : None
Download Size   : 2048.00 KiB
Installed Size  : 10.00 MiB
Install Reason  : Installed as a dependency for another package

Repository      : extra
Name            : ripgrep
Version         : 15.2.0-1
Description     : Search tool
Depends On      : None
Optional Deps   : None
Installed Size  : 3584.00 KiB
Install Reason  : Explicitly installed
";

    #[test]
    fn test_parse_stanzas_and_fields() {
        let stanzas = parse_stanzas(SAMPLE_INFO);
        assert_eq!(stanzas.len(), 2);

        let bash = &stanzas[0];
        assert_eq!(field_str(bash, "Name").as_deref(), Some("bash"));
        assert_eq!(field_str(bash, "Conflicts With"), None);
        assert_eq!(
            field_tokens(bash, "Depends On"),
            vec!["readline", "libreadline.so", "glibc", "ncurses"]
        );
        assert_eq!(
            field_tokens(bash, "Required By"),
            vec!["7zip", "base", "pacman"]
        );
        assert_eq!(field_optdeps(bash), vec!["bash-completion", "less"]);
        assert_eq!(
            parse_install_reason(bash).as_deref(),
            Some("installed as a dependency")
        );

        let model = model_from_stanza(bash, "core".to_string(), true);
        assert_eq!(model.name, "bash");
        assert_eq!(model.version, "5.3.20-2");
        assert!(model.is_installed);
        assert!((model.size_mb - 10.0).abs() < f64::EPSILON);

        let rg = &stanzas[1];
        assert!(field_tokens(rg, "Depends On").is_empty());
        assert!(field_optdeps(rg).is_empty());
        assert_eq!(
            parse_install_reason(rg).as_deref(),
            Some("explicitly installed")
        );
        assert!((parse_size_mb(field_str(rg, "Installed Size")) - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_size_units_and_regex_escape() {
        assert!((parse_size_mb(Some("1048576.00 B".into())) - 1.0).abs() < f64::EPSILON);
        assert!((parse_size_mb(Some("512.00 KiB".into())) - 0.5).abs() < f64::EPSILON);
        assert!((parse_size_mb(Some("2.00 GiB".into())) - 2048.0).abs() < f64::EPSILON);
        assert_eq!(parse_size_mb(None), 0.0);

        assert_eq!(escape_regex("gtk+3.0[x]"), r"gtk\+3\.0\[x\]");
        assert_eq!(strip_dep_constraint("foo>=1.2.3-1"), "foo");
    }
}
