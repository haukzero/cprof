use crate::error::Result;
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

pub fn run(target: &TargetSpec, names: Vec<String>, force: bool) -> Result<()> {
    let names = if names.is_empty() {
        vec![prompt::select_profile(
            target,
            None,
            "Profile name to remove (type to search)",
        )?]
    } else {
        storage::resolve_names(target, &names)?
    };

    for name in &names {
        storage::require_exists(target, name)?;
    }

    let active = activation::active_name(target)?;
    let selected_active = active
        .as_deref()
        .filter(|active| names.iter().any(|name| name == active));
    let skip_active = if let Some(active) = selected_active
        && !force
    {
        style::warning(format!("'{}' is the currently active profile!", active));
        if !prompt::confirm("Remove anyway? This will also remove active links")? {
            println!("Skipped '{}'", active);
            true
        } else {
            false
        }
    } else {
        false
    };

    for name in names {
        let is_active = active.as_deref() == Some(name.as_str());
        if is_active && skip_active {
            continue;
        }
        activation::remove_profile_links(target, &name)?;
        storage::delete(target, &name)?;
        println!(
            "Removed profile '{}'{}",
            name,
            if is_active {
                " and cleared active links"
            } else {
                ""
            }
        );
    }
    Ok(())
}
