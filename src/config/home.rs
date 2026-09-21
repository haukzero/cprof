use std::path::PathBuf;
use std::sync::OnceLock;

use crate::error::{AppError, Result};

static OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn dir() -> Result<PathBuf> {
    OVERRIDE
        .get()
        .cloned()
        .or_else(dirs::home_dir)
        .ok_or(AppError::NoHomeDir)
}

#[cfg(windows)]
pub(crate) fn set_override(path: PathBuf) -> Result<()> {
    if !path.is_absolute() {
        return Err(AppError::UnsafePath(path.display().to_string()));
    }
    OVERRIDE
        .set(path)
        .map_err(|_| AppError::ElevationAlreadyInitialized)
}

#[cfg(test)]
mod tests {
    use super::dir;

    #[test]
    fn current_home_is_available() {
        assert!(dir().is_ok());
    }
}
