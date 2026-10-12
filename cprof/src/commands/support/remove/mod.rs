use crate::commands::support::{selection::Scope, write::WriteSession};
use crate::error::Result;
use crate::filesystem::transaction::PathTransaction;
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
    WriteSession::targets(
        selected
            .iter()
            .map(|selection| selection.target.id.as_str()),
    )?
    .run(|| {
        // Selection may have waited for input while another process changed a
        // profile. Recheck the entire selection before deleting from any target.
        for selection in &selected {
            for name in &selection.profiles {
                storage::require_exists(&selection.target, name)?;
            }
        }
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

        PathTransaction::prepare(|transaction| {
            for (selection, _) in &removals {
                for name in &selection.profiles {
                    activation::LinkPlan::remove_profile(&selection.target, name)?
                        .stage(transaction)?;
                    storage::stage_delete(transaction, &selection.target, name)?;
                }
            }
            Ok(())
        })?
        .commit()?;

        for (selection, active) in removals {
            for name in selection.profiles {
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
    })
}
