use std::fs;

use crate::config;
use crate::error::{AppError, IoContext, ProfileError, Result, TransactionError};
use crate::profile::{self, activation, storage};
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

pub fn run(target: &TargetSpec, old_name: Option<String>, new_name: Option<String>) -> Result<()> {
    let (old_name, new_name) = match (old_name, new_name) {
        (Some(old_name), Some(new_name)) => (old_name, new_name),
        _ => (
            prompt::select_profile(target, None, "Profile to rename (type to search)")?,
            prompt::input("New profile name")?,
        ),
    };

    storage::require_exists(target, &old_name)?;
    profile::validate_name(&new_name)?;
    let new_dir = config::profile_dir(target, &new_name)?;
    if new_dir.exists() {
        return Err(ProfileError::Exists(new_name).into());
    }

    let old_dir = config::profile_dir(target, &old_name)?;
    let was_active = activation::active_name(target)?.as_deref() == Some(old_name.as_str());

    fs::rename(&old_dir, &new_dir).with_path(&old_dir)?;
    if was_active && let Err(error) = activation::switch(target, &new_name, false) {
        // The links already refer to new_dir when only backup cleanup failed.
        if matches!(
            error,
            AppError::Transaction(TransactionError::CleanupFailed(_))
        ) {
            return Err(error);
        }
        let rollback = fs::rename(&new_dir, &old_dir).with_path(&new_dir);
        return Err(error.with_recovery(rollback.err()));
    }

    style::success(format!("Renamed profile '{old_name}' to '{new_name}'"));
    Ok(())
}
