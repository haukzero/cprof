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
    adoption::adopt(target, &name).or_else(|error| {
        elevate::retry_as_admin(
            error,
            &[
                target.id.clone().into(),
                "adopt".into(),
                name.clone().into(),
            ],
        )
    })?;
    if !elevate::is_elevated_child() {
        style::success(format!("Adopted and activated profile '{name}'"));
    }
    Ok(())
}
