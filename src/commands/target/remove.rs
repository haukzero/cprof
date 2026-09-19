use crate::activation;
use crate::error::Result;
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec, names: Vec<String>) -> Result<()> {
    let names = if names.is_empty() {
        vec![prompt::select_profile(
            target,
            None,
            "Profile name to remove (type to search)",
        )?]
    } else {
        profile::resolve_names(target, &names)?
    };
    for name in names {
        prompt::require_profile(target, &name)?;
        let active = activation::active_name(target)?.as_deref() == Some(name.as_str());
        if active {
            println!(
                "{}",
                style::warning(&format!("'{}' is the currently active profile!", name))
            );
            if !prompt::confirm("Remove anyway? This will also remove active links")? {
                println!("Skipped '{}'", name);
                continue;
            }
        }
        activation::remove_profile_links(target, &name)?;
        profile::delete(target, &name)?;
        println!(
            "Removed profile '{}'{}",
            name,
            if active {
                " and cleared active links"
            } else {
                ""
            }
        );
    }
    Ok(())
}
