fn main() {
    let handle = alpm::Alpm::new("/", "/var/lib/pacman").unwrap();
    let db = handle.localdb();
    let pkg = db.pkg("bash").unwrap();
    for conflict in pkg.conflicts() {
        let version_str = conflict.version().map_or("".to_string(), |v| v.as_str().to_string());
        println!("{} {}", conflict.name(), version_str);
    }
}
