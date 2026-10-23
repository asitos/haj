use crate::backend::traits::{HistoryEntry, InstallSummary, TransactionAction};
use crate::core::package::PackageModel;

fn strip_arch(name: &str) -> String {
    name.split(':').next().unwrap_or(name).to_string()
}

pub fn parse_apt_cache_search(stdout: &str) -> Vec<PackageModel> {
    let mut results = Vec::new();
    for line in stdout.lines() {
        if let Some((name, _desc)) = line.split_once(" - ") {
            results.push(PackageModel {
                name: strip_arch(name.trim()),
                version: "Unknown".to_string(),
                repo: "apt".to_string(),
                is_installed: false,
                is_upgradable: false,
                size_mb: 0.0,
                desc: Some(_desc.to_string()),
            });
        }
    }
    results
}

pub fn parse_apt_cache_show(stdout: &str, package: &str) -> Option<PackageModel> {
    let mut version = String::new();
    let mut size_mb = 0.0;
    
    for line in stdout.lines() {
        if line.trim().is_empty() {
            if !version.is_empty() {
                break; // end of first stanza
            }
            continue;
        }
        if let Some(v) = line.strip_prefix("Version: ") {
            if version.is_empty() {
                version = v.trim().to_string();
            }
        } else if let Some(s) = line.strip_prefix("Size: ") {
            if let Ok(bytes) = s.trim().parse::<f64>() {
                size_mb = bytes / 1_048_576.0;
            }
        }
    }
    
    if version.is_empty() {
        None
    } else {
        Some(PackageModel {
            name: strip_arch(package),
            version,
            repo: "apt".to_string(),
            is_installed: false,
            is_upgradable: false,
            size_mb, desc: None,
        })
    }
}

pub fn parse_dpkg_query_list(stdout: &str) -> Vec<PackageModel> {
    let mut results = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 && parts[2].contains("installed") {
            results.push(PackageModel {
                name: strip_arch(parts[0]),
                version: parts[1].to_string(),
                repo: "local".to_string(),
                is_installed: true,
                is_upgradable: false,
                size_mb: 0.0,
                desc: None,
            });
        }
    }
    results
}

pub fn parse_apt_get_install_dry_run(stdout: &str) -> Vec<InstallSummary> {
    let mut summaries = Vec::new();
    let mut in_new = false;
    
    for line in stdout.lines() {
        if line.starts_with("The following NEW packages will be installed:") {
            in_new = true;
            continue;
        } else if line.starts_with("The following packages will be") || line.starts_with("0 upgraded") {
            in_new = false;
        }
        
        if in_new && line.starts_with("  ") {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            let mut i = 0;
            while i < parts.len() {
                let name = strip_arch(parts[i]);
                let mut version = "Unknown".to_string();
                if i + 1 < parts.len() && parts[i+1].starts_with('(') && parts[i+1].ends_with(')') {
                    version = parts[i+1].trim_matches(|c| c == '(' || c == ')').to_string();
                    i += 2;
                } else {
                    i += 1;
                }
                summaries.push(InstallSummary {
                    action: TransactionAction::Install,
                    name, version, download_size_mb: 0.0, install_size_mb: 0.0
                });
            }
        }
    }
    summaries
}

pub fn parse_history_log(content: &str) -> Vec<HistoryEntry> {
    let mut entries = Vec::new();
    let mut current_date = String::new();
    
    for line in content.lines() {
        if let Some(date) = line.strip_prefix("Start-Date: ") {
            current_date = date.trim().to_string();
        } else if let Some(install) = line.strip_prefix("Install: ") {
            let pkgs = install.split("), ");
            for pkg in pkgs {
                let parts: Vec<&str> = pkg.split(" (").collect();
                if parts.len() >= 2 {
                    let name = strip_arch(parts[0].trim());
                    let ver = parts[1].trim_end_matches(')').to_string();
                    entries.push(HistoryEntry {
                        timestamp: current_date.clone(),
                        action: "installed".to_string(),
                        package: name,
                        version: ver,
                    });
                }
            }
        } else if let Some(upgrade) = line.strip_prefix("Upgrade: ") {
            let pkgs = upgrade.split("), ");
            for pkg in pkgs {
                let parts: Vec<&str> = pkg.split(" (").collect();
                if parts.len() >= 2 {
                    let name = strip_arch(parts[0].trim());
                    let versions: Vec<&str> = parts[1].split(", ").collect();
                    let ver = if versions.len() >= 2 { versions[1].trim_end_matches(')').to_string() } else { parts[1].trim_end_matches(')').to_string() };
                    entries.push(HistoryEntry {
                        timestamp: current_date.clone(),
                        action: "upgraded".to_string(),
                        package: name,
                        version: ver,
                    });
                }
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_apt_cache_search() {
        let output = "rustc - Rust compiler\nrust-gdb - Rust debugger (gdb)\n";
        let pkgs = parse_apt_cache_search(output);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "rustc");
        assert_eq!(pkgs[1].name, "rust-gdb");
    }

    #[test]
    fn test_parse_apt_cache_show() {
        let output = "Package: ripgrep\nVersion: 13.0.0-2\nArchitecture: amd64\nSize: 1234567\n";
        let pkg = parse_apt_cache_show(output, "ripgrep").unwrap();
        assert_eq!(pkg.name, "ripgrep");
        assert_eq!(pkg.version, "13.0.0-2");
        assert!((pkg.size_mb - (1234567.0 / 1_048_576.0)).abs() < 0.001);
    }

    #[test]
    fn test_parse_dpkg_query_list() {
        let output = "bash\t5.1-6ubuntu1\tinstall ok installed\ncurl\t7.81.0\tinstall ok installed\n";
        let pkgs = parse_dpkg_query_list(output);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "bash");
        assert_eq!(pkgs[0].version, "5.1-6ubuntu1");
        assert_eq!(pkgs[1].name, "curl");
    }

    #[test]
    fn test_parse_apt_get_install_dry_run() {
        let output = "Reading package lists... Done\nBuilding dependency tree... Done\nThe following NEW packages will be installed:\n  ripgrep (13.0.0-2) fzf (0.29.0-1)\n0 upgraded, 2 newly installed, 0 to remove and 0 not upgraded.\n";
        let summaries = parse_apt_get_install_dry_run(output);
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].name, "ripgrep");
        assert_eq!(summaries[0].version, "13.0.0-2");
        assert_eq!(summaries[1].name, "fzf");
        assert_eq!(summaries[1].version, "0.29.0-1");
    }

    #[test]
    fn test_parse_apt_get_install_dry_run_wrapped() {
        let output = "Reading package lists... Done\nBuilding dependency tree... Done\nThe following NEW packages will be installed:\n  fzf ripgrep zsh\n0 upgraded, 3 newly installed, 0 to remove.\n";
        let summaries = parse_apt_get_install_dry_run(output);
        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].name, "fzf");
        assert_eq!(summaries[0].version, "Unknown");
        assert_eq!(summaries[1].name, "ripgrep");
        assert_eq!(summaries[2].name, "zsh");
    }

    #[test]
    fn test_strip_arch() {
        let output = "bash:amd64\t5.1\tinstall ok installed\n";
        let pkgs = parse_dpkg_query_list(output);
        assert_eq!(pkgs[0].name, "bash");
    }


    #[test]
    fn test_parse_history_log() {
        let output = "Start-Date: 2023-10-01  12:00:00\nCommandline: apt-get install vim\nInstall: vim:amd64 (2:8.2.3995-1ubuntu2.1)\nEnd-Date: 2023-10-01  12:00:05\n\nStart-Date: 2023-10-02  14:00:00\nCommandline: apt-get upgrade\nUpgrade: curl:amd64 (7.81.0-1, 7.81.0-1ubuntu1.2)\nEnd-Date: 2023-10-02  14:00:10\n";
        let history = parse_history_log(output);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].action, "installed");
        assert_eq!(history[0].package, "vim");
        assert_eq!(history[0].version, "2:8.2.3995-1ubuntu2.1");
        assert_eq!(history[1].action, "upgraded");
        assert_eq!(history[1].package, "curl");
        assert_eq!(history[1].version, "7.81.0-1ubuntu1.2");
    }
}
