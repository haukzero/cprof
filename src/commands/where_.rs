use crate::config;
use crate::error::Result;
use crate::prompt;

pub fn run(name: Option<String>) -> Result<()> {
    // Get the profile name (with fuzzy select if not provided)
    let name = prompt::select_profile(name, "Profile name to switch to (type to search)")?;

    // Validate profile exists
    prompt::require_profile(&name)?;

    // Get the profile filepath
    let path = config::profile_settings(&name)?;
    println!("{}", path.display());

    Ok(())
}
