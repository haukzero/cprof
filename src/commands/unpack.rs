use std::fs;
use std::path::Path;

use dialoguer::Confirm;

use crate::config;
use crate::error::{AppError, Result};
use crate::package;
use crate::profile;
use crate::style;

pub fn run(path: Option<String>, force: bool) -> Result<()> {
    let pkg_path = path.unwrap_or_else(|| "cprof.pkg".to_string());
    let pkg = Path::new(&pkg_path);

    if !pkg.exists() {
        return Err(AppError::Other(format!(
            "Package file '{}' not found",
            pkg.display()
        )));
    }

    // Read and decode the package
    let data = fs::read(pkg)?;
    let entries = package::decode(&data)?;

    // Get active name once before the loop
    let active_name = profile::get_active_name()?;

    let mut unpacked = 0;
    let mut skipped = 0;

    for entry in &entries {
        // Check if profile already exists
        let profile_path = config::profile_settings(&entry.name)?;
        if profile_path.exists() {
            let is_active = active_name.as_deref() == Some(entry.name.as_str());
            let label = if is_active {
                format!("{} (active)", entry.name)
            } else {
                entry.name.clone()
            };

            println!(
                "{}",
                style::warning(&format!("Profile '{}' already exists - conflict!", label))
            );

            // Determine whether to overwrite
            let overwrite = if force {
                true
            } else if !atty::is(atty::Stream::Stdin) {
                println!("Non-interactive mode: skipping '{}'", label);
                false
            } else {
                Confirm::new()
                    .with_prompt(format!("Overwrite '{}'?", label))
                    .default(false)
                    .interact()
                    .map_err(|e| AppError::Other(e.to_string()))?
            };

            if !overwrite {
                println!("Skipped '{}'", label);
                skipped += 1;
                continue;
            }

            // Remove existing profile - remove symlink if active
            if is_active {
                let link = config::settings_link()?;
                if link.exists() || fs::symlink_metadata(&link).is_ok() {
                    fs::remove_file(&link)?;
                }
            }
            fs::remove_dir_all(profile_path.parent().unwrap())?;
        }

        // Create the profile
        profile::create_profile(&entry.name, &entry.content)?;
        unpacked += 1;
        println!("Unpacked '{}'", entry.name);
    }

    println!("\nDone: {} unpacked, {} skipped", unpacked, skipped);

    Ok(())
}
