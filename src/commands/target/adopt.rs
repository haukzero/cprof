use crate::elevate;
use crate::error::Result;
use crate::profile::adoption;
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

pub fn run(target: &TargetSpec, name: Option<String>) -> Result<()> {
    let name = match name {
        Some(name) => name,
        None => prompt::input("New profile name")?,
    };
    elevate::run(
        &[
            target.id.clone().into(),
            "adopt".into(),
            name.clone().into(),
        ],
        || adoption::adopt(target, &name),
    )?;
    if !elevate::is_elevated_child() {
        style::success(format!("Adopted and activated profile '{name}'"));
    }
    Ok(())
}
