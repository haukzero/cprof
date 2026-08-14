use crate::error::Result;
use crate::profile;
use crate::prompt;
use crate::style;

pub fn run(names: Vec<String>) -> Result<()> {
    let names = if names.is_empty() {
        // Interactive mode: let user select with fuzzy search
        let name = prompt::select_profile(None, "Profile name to remove (type to search)")?;
        vec![name]
    } else {
        names
    };

    for name in &names {
        // Validate profile exists
        prompt::require_profile(name)?;

        // Check if it's the active profile
        let is_active = profile::get_active_name()?.as_deref() == Some(name.as_str());

        if is_active {
            println!(
                "{}",
                style::warning(&format!("'{}' is the currently active profile!", name))
            );

            if !prompt::confirm("Remove anyway? This will also remove the symlink")? {
                println!("Skipped '{}'", name);
                continue;
            }
        }

        profile::remove_profile(name)?;

        if is_active {
            println!("Removed profile '{}' and cleared active symlink", name);
        } else {
            println!("Removed profile '{}'", name);
        }
    }

    Ok(())
}
