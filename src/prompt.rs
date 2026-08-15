use dialoguer::{Confirm, FuzzySelect};

use crate::config;
use crate::error::{AppError, Result};
use crate::profile;

/// Select a profile using fuzzy search
///
/// If `name` is Some, returns it directly.
/// If `name` is None, shows a FuzzySelect for the user to pick.
pub fn select_profile(name: Option<String>, prompt: &str) -> Result<String> {
    match name {
        Some(n) => Ok(n),
        None => {
            let profiles = profile::list_profiles()?;
            if profiles.is_empty() {
                return Err(AppError::ProfileNotFound("(no profiles exist)".to_string()));
            }

            // Build display labels with an active indicator
            let labels: Vec<String> = profiles
                .iter()
                .map(|p| {
                    if p.active {
                        format!("{} (active)", p.name)
                    } else {
                        p.name.clone()
                    }
                })
                .collect();

            let selection = FuzzySelect::new()
                .with_prompt(prompt)
                .items(&labels)
                .interact()
                .map_err(|e| AppError::Other(e.to_string()))?;

            Ok(profiles[selection].name.clone())
        }
    }
}

/// Validate that a profile exists
pub fn require_profile(name: &str) -> Result<()> {
    let profile_dir = config::profiles_dir()?.join(name);
    if !profile_dir.exists() {
        return Err(AppError::ProfileNotFound(name.to_string()));
    }
    let profile_file = profile_dir.join("settings.json");
    if !profile_file.exists() {
        return Err(AppError::ProfileNotFound(name.to_string()));
    }
    Ok(())
}

/// Ask for confirmation with a yes/no prompt
pub fn confirm(prompt: &str) -> Result<bool> {
    Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .map_err(|e| AppError::Other(e.to_string()))
}
