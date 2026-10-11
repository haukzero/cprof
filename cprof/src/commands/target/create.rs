use crate::cli::EditorOptions;
use crate::commands::support::create;
use crate::error::Result;
use crate::targets::TargetSpec;
use crate::ui::{prompt, style};

#[cprof_macros::command(retry = created.activate, args = [&target.id, "switch", &name])]
pub fn run(
    target: &TargetSpec,
    name: Option<String>,
    copy_from: Option<String>,
    editor: EditorOptions,
) -> Result<()> {
    let name = match name {
        Some(name) => name,
        None => prompt::input("Profile name")?,
    };
    let copy_from = match copy_from {
        Some(source) => Some(source),
        None => prompt::select_copy_source(target)?,
    };
    let created = create::prepare(target, &name, copy_from.as_deref(), editor)?;
    let activated = created.activate()?;
    if activated == Some(false) {
        style::success(format!("Created profile '{}'", name));
    } else {
        style::success(format!("Created and activated profile '{}'", name));
    }
    Ok(())
}
