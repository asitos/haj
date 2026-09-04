#[derive(Clone, Debug)]
pub struct PackageModel {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub is_installed: bool,
    pub is_upgradable: bool,
    pub size_mb: f64,
}
