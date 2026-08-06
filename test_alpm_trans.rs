use alpm::Alpm;
fn main() {
    let mut handle = Alpm::new("/", "/var/lib/pacman").unwrap();
    let res: alpm::Error = handle.error(); 
}
