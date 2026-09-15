use std::fs;

use crate::editor;
use crate::error::{AppError, Result};
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::{ResourceSpec, TargetSpec};

pub fn run(
    target: &'static TargetSpec,
    name: Option<String>,
    filename: Option<String>,
    editor_name: Option<String>,
) -> Result<()> {
    let name = prompt::select_profile(target, name, "Profile to edit (type to search)")?;
    prompt::require_profile(target, &name)?;
    let resources: Vec<&'static ResourceSpec> = match filename {
        Some(filename) => vec![target.resource(&filename)?],
        None => target.resources.iter().collect(),
    };
    let editor_cmd = editor::resolve_editor(editor_name)?;
    let mut changed = false;
    for resource in resources {
        let path = profile::resource_path(target, &name, resource)?;
        if !path.exists() {
            if resource.required {
                return Err(AppError::IncompleteProfile(name.clone()));
            }
            continue;
        }
        let before = fs::read(&path)?;
        editor::open_editor(&editor_cmd, &path)?;
        let after = fs::read(&path)?;
        if after != before {
            profile::validate_resource_file(&path, resource)?;
            changed = true;
        }
    }
    if changed {
        println!("{} Updated profile '{}'", style::success("Done!"), name);
    } else {
        println!("No changes made to profile '{}'", name);
    }
    Ok(())
}
