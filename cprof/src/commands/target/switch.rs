use crate::commands::support::adopt;
use crate::elevate;
use crate::error::Result;
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

#[cprof_macros::command(
    retry = activation::switch,
    args = [&target.id, "switch", &name],
    flags = [(force, "--force")],
)]
pub fn run(target: &TargetSpec, name: Option<String>, force: bool) -> Result<()> {
    let status = activation::status(target)?;
    adopt::warn_unmanaged(&target.id, &status);
    if force && status == activation::Status::Unmanaged && !elevate::is_elevated_child() {
        style::warning("Switching with --force replaces unmanaged entries without saving them");
    }
    let name = prompt::select_profile(target, name, "Profile name to switch to (type to search)")?;
    storage::require_exists(target, &name)?;
    let already_active = activation::switch(target, &name, force)?;
    if elevate::is_elevated_child() {
        return Ok(());
    }
    if already_active == Some(true) {
        style::warning(format!("'{}' is already the active profile", name));
    } else {
        println!("Switched to profile '{}'", name);
    }
    Ok(())
}
