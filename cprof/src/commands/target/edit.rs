use crate::cli::EditorOptions;
use crate::commands::support::edit;
use crate::error::{ActivationError, Result};
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

#[cprof_macros::command(no_retry)]
pub fn run(
    target: &TargetSpec,
    name: Option<String>,
    active: bool,
    filename: Option<String>,
    editor: EditorOptions,
) -> Result<()> {
    let name = resolve_name(target, name, active)?;
    let changed = edit::profile(target, &name, filename.as_deref(), editor)?;
    if changed {
        style::success(format!("Updated profile '{}'", name));
    } else {
        println!("No changes made to profile '{}'", name);
    }
    Ok(())
}

fn resolve_name(target: &TargetSpec, name: Option<String>, active: bool) -> Result<String> {
    if let Some(name) = &name {
        storage::require_exists(target, name)?;
    }
    if !active {
        return prompt::select_profile(target, name, "Profile to edit (type to search)");
    }

    let active_name = activation::active_name(target)?
        .ok_or_else(|| ActivationError::NoActiveProfile(target.id.clone()))?;
    match name {
        Some(name) if name != active_name => {
            return Err(ActivationError::ProfileMismatch {
                target: target.id.clone(),
                requested: name,
                active: active_name,
            }
            .into());
        }
        Some(_) => style::warning(format!(
            "Profile '{active_name}' is already selected by --active; specifying its name is redundant"
        )),
        None => {}
    }
    Ok(active_name)
}
