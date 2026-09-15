use crate::activation;
use crate::elevate;
use crate::error::{AppError, Result};
use crate::prompt;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(target: &'static TargetSpec, name: Option<String>, force: bool) -> Result<()> {
    let name = prompt::select_profile(target, name, "Profile name to switch to (type to search)")?;
    prompt::require_profile(target, &name)?;
    let switched = match activation::switch(target, &name, force) {
        Ok(value) => value,
        Err(error) if elevate::is_privilege_error(&error) => {
            let mut args = vec![target.id.into(), "switch".into(), name.clone().into()];
            if force {
                args.push("--force".into());
            }
            if !elevate::run_as_admin(&args)? {
                return Err(AppError::ElevationFailed);
            }
            false
        }
        Err(error) => return Err(error),
    };
    if elevate::is_elevated_child() {
        return Ok(());
    }
    if switched {
        println!(
            "{}",
            style::warning(&format!("'{}' is already the active profile", name))
        );
    } else {
        println!("Switched to profile '{}'", name);
    }
    Ok(())
}
