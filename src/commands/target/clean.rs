use std::fs;

use crate::config;
use crate::error::{IoContext, Result};
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

pub fn run(target: &TargetSpec, force: bool) -> Result<()> {
    let profiles = storage::names(target)?;
    let active = activation::active_name(target)?;
    if profiles.is_empty() {
        println!("No profiles to clean.");
        return Ok(());
    }
    style::warning("This will remove all profiles and active links:");
    for name in &profiles {
        println!(
            "  {}{}",
            name,
            if active.as_deref() == Some(name.as_str()) {
                format!(" {}", style::active_tag())
            } else {
                String::new()
            }
        );
    }
    if !force && !prompt::confirm("Continue?")? {
        println!("Cancelled.");
        return Ok(());
    }
    for name in &profiles {
        let _ = activation::remove_profile_links(target, name)?;
    }
    let dir = config::profiles_dir(target)?;
    if dir.exists() {
        fs::remove_dir_all(&dir).with_path(&dir)?;
    }
    style::success(format!("Cleaned {} profile(s)", profiles.len()));
    Ok(())
}
