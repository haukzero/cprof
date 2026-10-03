use crate::commands::support::selection::Scope;
use crate::error::Result;
use crate::profile::{activation, storage};
use crate::targets::TargetRepository;
use crate::ui::tree::TreeStyle;
use crate::ui::{prompt, style};

mod selection;

pub(crate) fn run(
    targets: &TargetRepository,
    scope: Scope<'_>,
    names: Vec<String>,
    force: bool,
    ascii: bool,
) -> Result<()> {
    let selected = selection::resolve(targets, scope, names, TreeStyle::resolve(ascii))?;
    let mut removals = Vec::new();
    // Finish all active-profile confirmations before deleting from any target.
    for mut selection in selected {
        if selection.profiles.is_empty() {
            continue;
        }
        let active = activation::active_name(&selection.target)?;
        if let Some(active) = active.as_deref()
            && selection.profiles.contains(active)
            && !force
        {
            let label = scope.label(&selection.target, active);
            style::warning(format!("'{label}' is the currently active profile!"));
            if !prompt::confirm("Remove anyway? This will also remove active links")? {
                println!("Skipped '{label}'");
                selection.profiles.shift_remove(active);
            }
        }
        removals.push((selection, active));
    }

    for (selection, active) in removals {
        for name in selection.profiles {
            activation::remove_profile_links(&selection.target, &name)?;
            storage::delete(&selection.target, &name)?;
            println!(
                "Removed profile '{}'{}",
                scope.label(&selection.target, &name),
                if active.as_deref() == Some(name.as_str()) {
                    " and cleared active links"
                } else {
                    ""
                }
            );
        }
    }
    Ok(())
}
