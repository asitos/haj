use alpm::Alpm;

#[test]
fn test_alpm_conflicts() {
    let handle = Alpm::new("/", "/var/lib/pacman").unwrap();
    let db = handle.localdb();

    // bash is usually installed, but if not this might panic in CI.
    // we should just safely ignore if it's not found
    if let Ok(pkg) = db.pkg("bash") {
        for conflict in pkg.conflicts() {
            let version_str = conflict
                .version()
                .map_or("".to_string(), |v| v.as_str().to_string());
            println!("{} {}", conflict.name(), version_str);
        }
    }
}

#[test]
fn test_alpm_depends() {
    let handle = Alpm::new("/", "/var/lib/pacman").unwrap();
    let db = handle.localdb();
    if let Ok(pkg) = db.pkg("bash") {
        for dep in pkg.depends() {
            let mod_str = match dep.depmod() {
                alpm::DepMod::Any => "",
                alpm::DepMod::Eq => "=",
                alpm::DepMod::Ge => ">=",
                alpm::DepMod::Le => "<=",
                alpm::DepMod::Gt => ">",
                alpm::DepMod::Lt => "<",
            };
            println!(
                "{} {}{}",
                dep.name(),
                mod_str,
                dep.version().map_or("", |v| v.as_str())
            );
        }
    }
}

#[test]
fn test_alpm_error() {
    let handle = Alpm::new("/", "/var/lib/pacman").unwrap();
    let _res: alpm::Error = handle.last_error();
}
