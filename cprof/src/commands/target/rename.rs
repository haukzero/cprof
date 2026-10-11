use crate::commands::support::rename;
use crate::elevate;
use crate::error::Result;
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

#[cprof_macros::command(
    retry = rename::run,
    args = [&target.id, "rename", &old_name, &new_name],
)]
pub fn run(target: &TargetSpec, old_name: Option<String>, new_name: Option<String>) -> Result<()> {
    let (old_name, new_name) = match (old_name, new_name) {
        (Some(old_name), Some(new_name)) => (old_name, new_name),
        _ => (
            prompt::select_profile(target, None, "Profile to rename (type to search)")?,
            prompt::input("New profile name")?,
        ),
    };

    rename::run(target, &old_name, &new_name)?;

    if !elevate::is_elevated_child() {
        style::success(format!("Renamed profile '{old_name}' to '{new_name}'"));
    }
    Ok(())
}
