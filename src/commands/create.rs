use std::fs;

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

    // Create profile with template content
    profile::create_profile(&name, &initial_content)?;

    // Resolve editor and open the actual profile file
    let editor_cmd = editor::resolve_editor(editor_name)?;
    if let Err(e) = editor::open_editor(&editor_cmd, &profile_path) {
        let _ = profile::remove_profile(&name);
        return Err(e);
    }

    // Read back and validate JSON
    let content = fs::read_to_string(&profile_path)?;
    if let Err(e) = serde_json::from_str::<serde_json::Value>(&content) {
        let _ = profile::remove_profile(&name);
        return Err(AppError::Json(e));
    }

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
