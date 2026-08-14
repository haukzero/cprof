use std::fs;

use crate::config;
use crate::editor;
use crate::error::Result;
use crate::prompt;
use crate::style;

pub fn run(name: Option<String>, editor_name: Option<String>) -> Result<()> {
    // Get the profile name (with fuzzy select if not provided)
    let name = prompt::select_profile(name, "Profile to edit (type to search)")?;

    // Validate profile exists
    prompt::require_profile(&name)?;

    // Read current content
    let profile_path = config::profile_settings(&name)?;
    let current_content = fs::read_to_string(&profile_path)?;

    // Edit content
    let new_content = editor::edit_content(&name, &current_content, editor_name)?;

    // Check if content changed
    if new_content == current_content {
        println!("No changes made to profile '{}'", name);
        return Ok(());
    }

    // Validate it's valid JSON
    serde_json::from_str::<serde_json::Value>(&new_content)?;

    // Save the updated content
    fs::write(&profile_path, &new_content)?;

    println!("{} Updated profile '{}'", style::success("Done!"), name);

    Ok(())
}
