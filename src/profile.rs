use std::fs;
use std::path::{Path, PathBuf};

use crate::config;
use crate::error::{AppError, Result};

/// The status of settings.json — used by commands that need to distinguish
/// *why* there is no active profile (e.g. `which`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsStatus {
    /// settings.json is a symlink to a managed profile
    Active(String),
    /// settings.json doesn't exist
    NoFile,
    /// settings.json exists but is a regular file, not a cprof symlink
    NotManaged,
    /// settings.json is a symlink pointing outside the profiles directory
    ExternalSymlink(String),
}

/// A profile entry: name and whether it's active
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    pub name: String,
    pub active: bool,
}

/// List all profiles, marking the active one
pub fn list_profiles() -> Result<Vec<ProfileInfo>> {
    let dir = config::profiles_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let active = get_active_name()?;
    let mut profiles = Vec::new();

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Only include directories that contain settings.json
            if entry.path().join("settings.json").exists() {
                profiles.push(ProfileInfo {
                    name: name.clone(),
                    active: active.as_deref() == Some(&name),
                });
            }
        }
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(profiles)
}

/// Get the full status of settings.json
pub fn get_settings_status() -> Result<SettingsStatus> {
    let link = config::settings_link()?;

    // Check if file exists
    if !link.exists() {
        // Check if the symlink exists but points to a missing target
        if fs::symlink_metadata(&link).is_ok() {
            // Symlink exists but target is missing - could be broken symlink
            if let Ok(target) = fs::read_link(&link) {
                let profiles_dir = config::profiles_dir()?;
                if !target.starts_with(&profiles_dir) {
                    return Ok(SettingsStatus::ExternalSymlink(
                        target.display().to_string(),
                    ));
                }
            }
        }
        return Ok(SettingsStatus::NoFile);
    }

    // Check if it's a symlink
    let metadata = fs::symlink_metadata(&link)?;
    if !metadata.file_type().is_symlink() {
        return Ok(SettingsStatus::NotManaged);
    }

    // Read the symlink target
    let target = fs::read_link(&link)?;

    // Extract profile name from path: ~/.claude-profiles/<name>/settings.json
    let profiles_dir = config::profiles_dir()?;
    if let Ok(rel) = target.strip_prefix(&profiles_dir)
        && let Some(name) = rel.iter().next()
    {
        return Ok(SettingsStatus::Active(name.to_string_lossy().to_string()));
    }

    // Symlink points somewhere else
    Ok(SettingsStatus::ExternalSymlink(
        target.display().to_string(),
    ))
}

/// Get the name of the currently active profile
///
/// Returns:
/// - `Ok(Some(name))` if settings.json is a symlink to a managed profile
/// - `Ok(None)` otherwise (no file, regular file, external symlink)
pub fn get_active_name() -> Result<Option<String>> {
    match get_settings_status()? {
        SettingsStatus::Active(name) => Ok(Some(name)),
        _ => Ok(None),
    }
}

/// Create a new profile with the given name and settings content
pub fn create_profile(name: &str, content: &str) -> Result<PathBuf> {
    let profile_dir = config::profiles_dir()?.join(name);
    if profile_dir.exists() {
        return Err(AppError::ProfileExists(name.to_string()));
    }

    fs::create_dir_all(&profile_dir)?;
    let settings_path = profile_dir.join("settings.json");
    fs::write(&settings_path, content)?;

    Ok(settings_path)
}

/// Remove a profile by name. Returns true if it was the active profile.
pub fn remove_profile(name: &str) -> Result<bool> {
    let profile_dir = config::profiles_dir()?.join(name);
    if !profile_dir.exists() {
        return Err(AppError::ProfileNotFound(name.to_string()));
    }

    let was_active = get_active_name()?.as_deref() == Some(name);

    // If active, remove the symlink first
    if was_active {
        let link = config::settings_link()?;
        if link.exists() {
            fs::remove_file(&link)?;
        }
    }

    fs::remove_dir_all(&profile_dir)?;
    Ok(was_active)
}

/// Switch to a profile. Returns true if it was already active.
pub fn switch_profile(name: &str) -> Result<bool> {
    let profile_dir = config::profiles_dir()?.join(name);
    if !profile_dir.exists() {
        return Err(AppError::ProfileNotFound(name.to_string()));
    }

    let current = get_active_name()?;
    if current.as_deref() == Some(name) {
        return Ok(true); // Already active
    }

    let link = config::settings_link()?;
    let target = profile_dir.join("settings.json");

    // Remove existing symlink if present
    if link.exists() || fs::symlink_metadata(&link).is_ok() {
        fs::remove_file(&link)?;
    }

    // Ensure parent directory exists
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }

    // Create symlink (cross-platform)
    create_symlink(&target, &link)?;

    Ok(false)
}

/// Create a symlink, cross-platform
#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::windows::fs::symlink_file(target, link)?;
    Ok(())
}

/// Read the content of a profile's settings.json
pub fn read_profile(name: &str) -> Result<String> {
    let path = config::profile_settings(name)?;
    if !path.exists() {
        return Err(AppError::ProfileNotFound(name.to_string()));
    }
    Ok(fs::read_to_string(&path)?)
}

/// Get the default editor
pub fn default_editor() -> Option<String> {
    std::env::var("EDITOR")
        .ok()
        .or_else(|| std::env::var("VISUAL").ok())
}

/// Check if a command exists in PATH
pub fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}
