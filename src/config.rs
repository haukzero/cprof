use std::path::PathBuf;

use crate::error::{AppError, Result};

/// Get the profiles directory: ~/.claude-profiles/
pub fn profiles_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or(AppError::NoHomeDir)?;
    Ok(home.join(".claude-profiles"))
}

/// Get the claude settings symlink path: ~/.claude/settings.json
pub fn settings_link() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or(AppError::NoHomeDir)?;
    Ok(home.join(".claude").join("settings.json"))
}

/// Get the path to a profile's settings.json
pub fn profile_settings(name: &str) -> Result<PathBuf> {
    Ok(profiles_dir()?.join(name).join("settings.json"))
}
