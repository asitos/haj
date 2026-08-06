use alpm::Alpm;
fn main() {
    let handle = Alpm::new("/", "/var/lib/pacman").unwrap();
    let db = handle.localdb();
    let pkg = db.pkg("bash").unwrap();
    for dep in pkg.depends() {
        let mod_str = match dep.depmod() {
            alpm::DepMod::Any => "",
            alpm::DepMod::Eq => "=",
            alpm::DepMod::Ge => ">=",
            alpm::DepMod::Le => "<=",
            alpm::DepMod::Gt => ">",
            alpm::DepMod::Lt => "<",
        };
        println!("{} {}{}", dep.name(), mod_str, dep.version().map_or("", |v| v.as_str()));
    }
}
