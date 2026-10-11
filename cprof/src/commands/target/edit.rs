use crate::cli::EditorOptions;
use crate::commands::support::edit;
use crate::error::Result;
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

#[cprof_macros::command(no_retry)]
pub fn run(
    target: &TargetSpec,
    name: Option<String>,
    filename: Option<String>,
    editor: EditorOptions,
) -> Result<()> {
    let name = prompt::select_profile(target, name, "Profile to edit (type to search)")?;
    let changed = edit::profile(target, &name, filename.as_deref(), editor)?;
    if changed {
        style::success(format!("Updated profile '{}'", name));
    } else {
        println!("No changes made to profile '{}'", name);
    }
    Ok(())
}
