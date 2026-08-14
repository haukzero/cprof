use dialoguer::Input;

use crate::config;
use crate::editor;
use crate::error::{AppError, Result};
use crate::profile;
use crate::style;

pub fn run(name: Option<String>, editor_name: Option<String>) -> Result<()> {
    // Get the profile name
    let name = match name {
        Some(n) => n,
        None => Input::new()
            .with_prompt("Profile name")
            .interact_text()
            .map_err(|e| AppError::Other(e.to_string()))?,
    };

    // Check if profile already exists
    let profile_path = config::profile_settings(&name)?;
    if profile_path.exists() {
        return Err(AppError::ProfileExists(name));
    }

    // Default template
    let template = serde_json::json!({
        "env": {},
        "permissions": {
            "allow": []
        }
    });
    let initial_content = serde_json::to_string_pretty(&template)?;

    // Edit content
    let content = editor::edit_content(&name, &initial_content, editor_name)?;

    // Validate it's valid JSON
    serde_json::from_str::<serde_json::Value>(&content)?;

    // Create the profile
    profile::create_profile(&name, &content)?;

    // If no active profile, switch to this one
    if profile::get_active_name()?.is_none() {
        profile::switch_profile(&name)?;
        println!(
            "{} Created and activated profile '{}'",
            style::success("Done!"),
            name
        );
    } else {
        println!("{} Created profile '{}'", style::success("Done!"), name);
    }

    Ok(())
}
