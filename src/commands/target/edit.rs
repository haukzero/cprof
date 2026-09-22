use crate::cli::EditorOptions;
use crate::error::{ProfileError, Result};
use crate::profile::storage;
use crate::targets::{ResourceSpec, TargetSpec};
use crate::ui::{editor::EditSession, prompt, style};

pub fn run(
    target: &TargetSpec,
    name: Option<String>,
    filename: Option<String>,
    editor: EditorOptions,
) -> Result<()> {
    let name = prompt::select_profile(target, name, "Profile to edit (type to search)")?;
    storage::require_exists(target, &name)?;
    let resources: Vec<&ResourceSpec> = match filename {
        Some(filename) => vec![target.resource(&filename)?],
        None => target.resources.iter().collect(),
    };
    let mut session = EditSession::new(editor.editor, editor.editor_args)?;
    for resource in resources {
        let path = storage::resource_path(target, &name, resource)?;
        if !path.exists() {
            if resource.required {
                return Err(ProfileError::Incomplete(name.clone()).into());
            }
            continue;
        }
        session.edit(&path, |bytes| resource.validate(bytes))?;
    }
    let changed = session.commit()?;
    if changed {
        style::success(format!("Updated profile '{}'", name));
    } else {
        println!("No changes made to profile '{}'", name);
    }
    Ok(())
}
