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

            let profile_names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();

            let selection = FuzzySelect::new()
                .with_prompt(prompt)
                .items(&profile_names)
                .interact()
                .map_err(|e| AppError::Other(e.to_string()))?;

            Ok(profile_names[selection].to_string())
        }
    }
}

/// Validate that a profile exists
pub fn require_profile(name: &str) -> Result<()> {
    let profile_dir = config::profiles_dir()?.join(name);
    if !profile_dir.exists() {
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
