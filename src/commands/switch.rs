use crate::activation;
use crate::error::Result;
use crate::prompt;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(target: &'static TargetSpec, name: Option<String>, force: bool) -> Result<()> {
    let name = prompt::select_profile(target, name, "Profile name to switch to (type to search)")?;
    prompt::require_profile(target, &name)?;
    if activation::switch(target, &name, force)? {
        println!(
            "{}",
            style::warning(&format!("'{}' is already the active profile", name))
        );
    } else {
        println!("Switched to profile '{}'", name);
    }
    Ok(())
}
