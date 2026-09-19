use crate::activation;
use crate::error::Result;
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec, names: Vec<String>, force: bool) -> Result<()> {
    let names = if names.is_empty() {
        vec![prompt::select_profile(
            target,
            None,
            "Profile name to remove (type to search)",
        )?]
    } else {
        profile::resolve_names(target, &names)?
    };

    for name in &names {
        prompt::require_profile(target, name)?;
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
        profile::delete(target, &name)?;
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
