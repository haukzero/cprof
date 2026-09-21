use crate::error::Result;
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;
use crate::ui::style;

pub fn run(target: &TargetSpec) -> Result<()> {
    let profiles = storage::list(target)?;
    let active = activation::active_name_from_profiles(target, &profiles)?;
    if profiles.is_empty() {
        println!("No profiles found.");
        return Ok(());
    }
    for p in &profiles {
        let is_active = active.as_deref() == Some(p.name.as_str());
        let suffix = if is_active {
            format!(" {}", style::active_tag())
        } else if !p.complete {
            " (incomplete)".to_string()
        } else {
            String::new()
        };
        if is_active {
            println!("{}{}", style::active_name(&p.name), suffix);
        } else {
            println!("{}{}", p.name, suffix);
        }
    }
    println!(
        "---------------------\n{} total",
        style::label(&profiles.len().to_string())
    );
    Ok(())
}
