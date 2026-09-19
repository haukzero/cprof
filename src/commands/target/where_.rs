use crate::error::Result;
use crate::profile;
use crate::prompt;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec, name: Option<String>, filename: Option<String>) -> Result<()> {
    let name = prompt::select_profile(target, name, "Profile name (type to search)")?;
    prompt::require_profile(target, &name)?;
    match filename {
        Some(filename) => println!(
            "{}",
            profile::resource_path(target, &name, target.resource(&filename)?)?.display()
        ),
        None => {
            for resource in &target.resources {
                let path = profile::resource_path(target, &name, resource)?;
                println!("{}", path.display());
            }
        }
    }
    Ok(())
}
