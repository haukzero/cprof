use std::fs;
use std::path::Path;

use crate::error::Result;

pub fn write_file(path: &Path, content: &[u8]) -> Result<()> {
    fs::write(path, content)?;
    Ok(())
}

pub fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

#[cfg(unix)]
pub fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(windows)]
pub fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::windows::fs::symlink_file(target, link)?;
    Ok(())
}
