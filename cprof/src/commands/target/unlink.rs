use crate::commands::support::write;
use crate::error::{ActivationError, Result};
use crate::profile::activation;
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

#[cprof_macros::command(no_retry)]
pub fn run(target: &TargetSpec, force: bool) -> Result<()> {
    write::target(target, || {
        let name = activation::active_name(target)?
            .ok_or_else(|| ActivationError::NoActiveProfile(target.id.clone()))?;
        if !force
            && !prompt::confirm(&format!(
                "Unlink active profile '{name}'? Stored profile files will be kept"
            ))?
        {
            println!("Cancelled.");
            return Ok(());
        }
        activation::remove_profile_links(target, &name)?;
        style::success(format!("Unlinked profile '{name}'"));
        Ok(())
    })
}
