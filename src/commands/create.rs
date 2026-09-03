use dialoguer::Input;

use crate::activation::{self, Status};
use crate::editor;
use crate::error::{AppError, Result};
use crate::profile;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(
    target: &'static TargetSpec,
    name: Option<String>,
    editor_name: Option<String>,
) -> Result<()> {
    let name = match name {
        Some(name) => name,
        None => Input::new()
            .with_prompt("Profile name")
            .interact_text()
            .map_err(|e| AppError::Other(e.to_string()))?,
    };
    profile::create(target, &name)?;
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
