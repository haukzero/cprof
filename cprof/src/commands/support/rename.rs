use std::fs;

use crate::config;
use crate::error::{AppError, IoContext, ProfileError, Result, TransactionError};
use crate::profile::{self, activation, storage};
use crate::targets::TargetSpec;

use super::write;

pub(crate) fn run(target: &TargetSpec, old_name: &str, new_name: &str) -> Result<()> {
    write::target(target, || {
        storage::require_exists(target, old_name)?;
        profile::validate_name(new_name)?;
        let new_dir = config::profile_dir(target, new_name)?;
        if new_dir.exists() {
            return Err(ProfileError::Exists(new_name.to_string()).into());
        }
        let old_dir = config::profile_dir(target, old_name)?;
        let was_active = activation::active_name(target)?.as_deref() == Some(old_name);

        fs::rename(&old_dir, &new_dir).with_path(&old_dir)?;
        if was_active && let Err(error) = activation::switch(target, new_name, false) {
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
        Ok(())
    })
}
