use std::fs;

use crate::activation;
use crate::config;
use crate::error::{AppError, IoContext, Result};
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec, old_name: Option<String>, new_name: Option<String>) -> Result<()> {
    let (old_name, new_name) = match (old_name, new_name) {
        (Some(old_name), Some(new_name)) => (old_name, new_name),
        _ => (
            prompt::select_profile(target, None, "Profile to rename (type to search)")?,
            prompt::input("New profile name")?,
        ),
    };

    profile::require_exists(target, &old_name)?;
    profile::validate_name(&new_name)?;
    let new_dir = config::profile_dir(target, &new_name)?;
    if new_dir.exists() {
        return Err(AppError::ProfileExists(new_name));
    }

    let old_dir = config::profile_dir(target, &old_name)?;
    let was_active = activation::active_name(target)?.as_deref() == Some(old_name.as_str());

    fs::rename(&old_dir, &new_dir).with_path(&old_dir)?;
    if was_active && let Err(error) = activation::switch(target, &new_name, false) {
        let rollback = fs::rename(&new_dir, &old_dir).with_path(&new_dir);
        return Err(match rollback {
            Ok(()) => error,
            Err(rollback_error) => AppError::Other(format!(
                "Failed to rename profile '{}': {error}; rollback failed: {rollback_error}",
                old_name
            )),
        });
    }

    style::success(format!("Renamed profile '{old_name}' to '{new_name}'"));
    Ok(())
}
