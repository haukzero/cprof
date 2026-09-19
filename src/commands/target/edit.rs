use crate::cli::EditorOptions;
use crate::editor::EditSession;
use crate::error::{AppError, Result};
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::{ResourceSpec, TargetSpec};

pub fn run(
    target: &TargetSpec,
    name: Option<String>,
    filename: Option<String>,
    editor: EditorOptions,
) -> Result<()> {
    let name = prompt::select_profile(target, name, "Profile to edit (type to search)")?;
    profile::require_exists(target, &name)?;
    let resources: Vec<&ResourceSpec> = match filename {
        Some(filename) => vec![target.resource(&filename)?],
        None => target.resources.iter().collect(),
    };
    let mut session = EditSession::new(editor.editor, editor.editor_args)?;
    for resource in resources {
        let path = profile::resource_path(target, &name, resource)?;
        if !path.exists() {
            if resource.required {
                return Err(AppError::IncompleteProfile(name.clone()));
            }
            continue;
        }
        session.edit(&path, resource.validate)?;
    }
    let changed = session.commit()?;
    if changed {
        style::success(format!("Updated profile '{}'", name));
    } else {
        println!("No changes made to profile '{}'", name);
    }
    Ok(())
}
