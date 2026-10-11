use crate::commands::support::write;
use crate::elevate;
use crate::error::Result;
use crate::profile::adoption;
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

#[cprof_macros::command(retry = write::target, args = [&target.id, "adopt", &name])]
pub fn run(target: &TargetSpec, name: Option<String>) -> Result<()> {
    let name = match name {
        Some(name) => name,
        None => prompt::input("New profile name")?,
    };
    write::target(target, || adoption::adopt(target, &name))?;
    if !elevate::is_elevated_child() {
        style::success(format!("Adopted and activated profile '{name}'"));
    }
    Ok(())
}
