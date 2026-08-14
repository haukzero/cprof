use crate::error::Result;
use crate::profile;
use crate::prompt;
use crate::style;

pub fn run(name: Option<String>) -> Result<()> {
    // Get the profile name (with fuzzy select if not provided)
    let name = prompt::select_profile(name, "Profile name to switch to (type to search)")?;

    // Validate profile exists
    prompt::require_profile(&name)?;

    // Switch
    let was_active = profile::switch_profile(&name)?;

    if was_active {
        println!(
            "{}",
            style::warning(&format!("'{}' is already the active profile", name))
        );
    } else {
        println!("Switched to profile '{}'", name);
    }

    Ok(())
}
