pub mod apt;
#[cfg(feature = "pacman")]
pub mod pacman;
pub mod traits;

pub use traits::{BackendError, Capabilities, CommandPlan, PackageManager};

pub fn detect_backend() -> Result<Box<dyn PackageManager>, BackendError> {
    #[cfg(feature = "pacman")]
    if std::path::Path::new("/usr/bin/pacman").exists() {
        return Ok(Box::new(pacman::PacmanBackend::new()?));
    }

    if std::path::Path::new("/usr/bin/apt-get").exists() {
        return Ok(Box::new(apt::AptBackend::new()?));
    }

    Err(BackendError::ExecutionError(
        1,
        "No supported package manager found.".to_string(),
    ))
}
