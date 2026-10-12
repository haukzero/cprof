use crate::config;
use crate::error::{ProfileError, Result};
use crate::filesystem::transaction::PathTransaction;
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;

use super::write;

pub(crate) fn run(target: &TargetSpec, old_name: &str, new_name: &str) -> Result<()> {
    write::target(target, || {
        storage::require_exists(target, old_name)?;
        let new_dir = config::profile_dir(target, new_name)?;
        if new_dir.exists() {
            return Err(ProfileError::Exists(new_name.to_string()).into());
        }
        let old_dir = config::profile_dir(target, old_name)?;
        let was_active = activation::active_name(target)?.as_deref() == Some(old_name);

        PathTransaction::prepare(|transaction| {
            transaction.stage_rename(&old_dir, &new_dir)?;
            if was_active {
                let resources = storage::resource_keys(target, old_name)?;
                activation::LinkPlan::activate(target, new_name, &resources, false)?
                    .stage(transaction)?;
            }
            Ok(())
        })?
        .commit()
    })
}
