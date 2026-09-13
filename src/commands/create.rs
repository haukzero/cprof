use dialoguer::Input;

use crate::activation::{self, Status};
use crate::editor;
use crate::error::{AppError, Result};
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(
    target: &'static TargetSpec,
    name: Option<String>,
    copy_from: Option<String>,
    editor_name: Option<String>,
) -> Result<()> {
    let (name, copy_from) = match name {
        Some(name) => (name, copy_from),
        None => {
            let name = Input::new()
                .with_prompt("Profile name")
                .interact_text()
                .map_err(|e| AppError::Other(e.to_string()))?;
            let copy_from = match copy_from {
                Some(source) => Some(source),
                None => prompt::select_copy_source(target)?,
            };
            (name, copy_from)
        }
    };
    profile::create(target, &name, copy_from.as_deref())?;
    let editor_cmd = editor::resolve_editor(editor_name)?;
    let result = (|| {
        for resource in target.resources {
            let path = profile::resource_path(target, &name, resource)?;
            editor::open_editor(&editor_cmd, &path)?;
            profile::validate_resource_file(&path, resource)?;
        }
        Ok::<(), AppError>(())
    })();
    if let Err(error) = result {
        let _ = profile::delete(target, &name);
        return Err(error);
    }

    if matches!(activation::status(target)?, Status::NoFiles) {
        activation::switch(target, &name, false)?;
        println!(
            "{} Created and activated profile '{}'",
            style::success("Done!"),
            name
        );
    } else {
        println!("{} Created profile '{}'", style::success("Done!"), name);
    }
    Ok(())
}
