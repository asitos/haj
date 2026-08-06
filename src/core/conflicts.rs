use alpm::Alpm;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConflictInfo {
    pub incoming_pkg: String,
    pub installed_pkg: String,
    pub constraint: Option<String>,
}

pub fn detect_conflicts(
    alpm_handle: &Alpm,
    native_pkgs: &[String],
    aur_conflicts_map: &HashMap<String, Vec<String>>,
) -> Vec<ConflictInfo> {
    let mut conflicts = Vec::new();
    let local_db = alpm_handle.localdb();

    for target in native_pkgs {
        for db in alpm_handle.syncdbs() {
            if let Ok(pkg) = db.pkg(target.as_str()) {
                for conflict in pkg.conflicts() {
                    let c_name = conflict.name();
                    if let Ok(installed) = local_db.pkg(c_name) {
                        let mod_str = match conflict.depmod() {
                            alpm::DepMod::Any => "",
                            alpm::DepMod::Eq => "=",
                            alpm::DepMod::Ge => ">=",
                            alpm::DepMod::Le => "<=",
                            alpm::DepMod::Gt => ">",
                            alpm::DepMod::Lt => "<",
                        };
                        let constraint = conflict
                            .version()
                            .map(|v| format!("{}{}", mod_str, v.as_str()));
                        conflicts.push(ConflictInfo {
                            incoming_pkg: target.clone(),
                            installed_pkg: installed.name().to_string(),
                            constraint,
                        });
                    }
                }
                break;
            }
        }
    }

    for (aur_pkg, aur_conflicts) in aur_conflicts_map {
        for constraint_str in aur_conflicts {
            let ops = ["<=", ">=", "==", "<", ">", "="];
            let mut c_name = constraint_str.as_str();
            let mut constraint_val = None;
            for op in ops {
                if let Some(idx) = constraint_str.find(op) {
                    c_name = &constraint_str[..idx];
                    constraint_val = Some(constraint_str[idx..].to_string());
                    break;
                }
            }
            if let Ok(installed) = local_db.pkg(c_name) {
                conflicts.push(ConflictInfo {
                    incoming_pkg: aur_pkg.clone(),
                    installed_pkg: installed.name().to_string(),
                    constraint: constraint_val,
                });
            }
        }
    }

    for installed in local_db.pkgs() {
        for conflict in installed.conflicts() {
            let c_name = conflict.name();

            if native_pkgs.iter().any(|p| p == c_name)
                || aur_conflicts_map.keys().any(|p| p == c_name)
            {
                let mod_str = match conflict.depmod() {
                    alpm::DepMod::Any => "",
                    alpm::DepMod::Eq => "=",
                    alpm::DepMod::Ge => ">=",
                    alpm::DepMod::Le => "<=",
                    alpm::DepMod::Gt => ">",
                    alpm::DepMod::Lt => "<",
                };
                let constraint = conflict
                    .version()
                    .map(|v| format!("{}{}", mod_str, v.as_str()));
                conflicts.push(ConflictInfo {
                    incoming_pkg: c_name.to_string(),
                    installed_pkg: installed.name().to_string(),
                    constraint,
                });
            }
        }
    }

    conflicts.sort();
    conflicts.dedup();
    conflicts
}
