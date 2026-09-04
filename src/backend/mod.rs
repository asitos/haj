pub mod pacman;
pub mod traits;

pub use traits::{BackendError, Capabilities, CommandPlan, PackageManager};

pub fn detect_backend() -> Result<Box<dyn PackageManager>, BackendError> {
    if std::path::Path::new("/usr/bin/pacman").exists() {
        Ok(Box::new(pacman::PacmanBackend::new()?))
    } else {
        Err(BackendError::ExecutionError(
            1,
            "No supported package manager found.".to_string(),
        ))
    }
}
