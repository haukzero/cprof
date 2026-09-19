use crate::activation;
use crate::error::{IoContext, Result};
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec, force: bool) -> Result<()> {
    let profiles = profile::list(target)?;
    let active = activation::active_name(target)?;
    if profiles.is_empty() {
        println!("No profiles to clean.");
        return Ok(());
    }
    println!(
        "{}",
        style::warning("This will remove all profiles and active links:")
    );
    for profile in &profiles {
        println!(
            "  {}{}",
            profile.name,
            if active.as_deref() == Some(profile.name.as_str()) {
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
    for profile in &profiles {
        let _ = activation::remove_profile_links(target, &profile.name)?;
    }
    let dir = crate::config::profiles_dir(target)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_path(&dir)?;
    }
    println!(
        "{} Cleaned {} profile(s)",
        style::success("Done!"),
        profiles.len()
    );
    Ok(())
}
