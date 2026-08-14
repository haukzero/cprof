use std::fs;
use std::path::Path;

use dialoguer::Confirm;

use crate::config;
use crate::error::{AppError, Result};
use crate::package;
use crate::profile;
use crate::style;

pub fn run(path: Option<String>) -> Result<()> {
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

    let mut unpacked = 0;
    let mut skipped = 0;

    for entry in &entries {
        // Check if profile already exists
        let profile_path = config::profile_settings(&entry.name)?;
        if profile_path.exists() {
            println!(
                "{}",
                style::warning(&format!(
                    "Profile '{}' already exists - conflict!",
                    entry.name
                ))
            );

            // Check if we're in an interactive terminal
            if atty::is(atty::Stream::Stdin) {
                let overwrite = Confirm::new()
                    .with_prompt(format!("Overwrite '{}'?", entry.name))
                    .default(false)
                    .interact()
                    .map_err(|e| AppError::Other(e.to_string()))?;

                if !overwrite {
                    println!("Skipped '{}'", entry.name);
                    skipped += 1;
                    continue;
                }
            } else {
                println!("Non-interactive mode: skipping '{}'", entry.name);
                skipped += 1;
                continue;
            }

            // Remove existing profile first
            profile::remove_profile(&entry.name)?;
        }

        // Create the profile
        profile::create_profile(&entry.name, &entry.content)?;
        unpacked += 1;
        println!("Unpacked '{}'", entry.name);
    }

    println!("\nDone: {} unpacked, {} skipped", unpacked, skipped);

    Ok(())
}
