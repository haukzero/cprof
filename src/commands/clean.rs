use std::fs;

use crate::config;
use crate::error::Result;
use crate::profile;
use crate::prompt;
use crate::style;

pub fn run(force: bool) -> Result<()> {
    let profiles = profile::list_profiles()?;

    if profiles.is_empty() {
        println!("No profiles to clean.");
        return Ok(());
    }

    // Show what will be deleted
    println!(
        "{}",
        style::warning("This will remove all profiles and the active symlink:")
    );
    for p in &profiles {
        if p.active {
            println!("  {} {}", p.name, style::active_tag());
        } else {
            println!("  {}", p.name);
        }
    }

    // Confirm unless force flag is set
    if !force && !prompt::confirm("Continue?")? {
        println!("Cancelled.");
        return Ok(());
    }

    // Remove symlink first
    let link = config::settings_link()?;
    if link.exists() || fs::symlink_metadata(&link).is_ok() {
        fs::remove_file(&link)?;
        println!("Removed active symlink");
    }

    // Remove all profile directories
    let dir = config::profiles_dir()?;
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
        println!("Removed profiles directory");
    }

    println!(
        "{} Cleaned {} profile(s)",
        style::success("Done!"),
        profiles.len()
    );

    Ok(())
}
