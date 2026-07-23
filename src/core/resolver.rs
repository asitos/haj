use alpm::Alpm;
use anyhow::Result;

pub struct InstallSummary {
    pub name: String,
    pub version: String,
    pub download_size_mb: f64,
    pub install_size_mb: f64,
}

pub fn get_install_summaries(alpm: &Alpm, targets: &[String]) -> Result<Vec<InstallSummary>> {
    let mut summaries = Vec::new();

    for target in targets {
        let mut found = false;

        for db in alpm.syncdbs() {
            if let Ok(pkg) = db.pkg(target.as_str()) {
                summaries.push(InstallSummary {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    download_size_mb: pkg.download_size() as f64 / 1024.0 / 1024.0,
                    install_size_mb: pkg.isize() as f64 / 1024.0 / 1024.0,
                });
                found = true;
                break;
            }
        }

        if !found {
            anyhow::bail!("package '{}' not found in any repository.", target);
        }
    }

    Ok(summaries)
}
