use crate::error::Result;
use crate::profile::storage;
use crate::targets::TargetSpec;
use crate::ui::prompt;

pub fn run(target: &TargetSpec, name: Option<String>, filename: Option<String>) -> Result<()> {
    let name = prompt::select_profile(target, name, "Profile name (type to search)")?;
    storage::require_exists(target, &name)?;
    match filename {
        Some(filename) => println!(
            "{}",
            storage::resource_path(target, &name, target.resource(&filename)?)?.display()
        ),
        None => {
            for resource in &target.resources {
                let path = storage::resource_path(target, &name, resource)?;
                println!("{}", path.display());
            }
        }
    }
    Ok(())
}
