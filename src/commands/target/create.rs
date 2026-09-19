use crate::activation::{self, Status};
use crate::cli::EditorOptions;
use crate::editor::EditSession;
use crate::error::{AppError, Result};
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(
    target: &TargetSpec,
    name: Option<String>,
    copy_from: Option<String>,
    editor: EditorOptions,
) -> Result<()> {
    let (name, copy_from) = match name {
        Some(name) => (name, copy_from),
        None => {
            let name = prompt::input("Profile name")?;
            let copy_from = match copy_from {
                Some(source) => Some(source),
                None => prompt::select_copy_source(target)?,
            };
            (name, copy_from)
        }
    };
    profile::create(target, &name, copy_from.as_deref())?;
    let result = (|| {
        let mut session = EditSession::new(editor.editor, editor.editor_args)?;
        for resource in &target.resources {
            let path = profile::resource_path(target, &name, resource)?;
            session.edit(&path, resource.validate)?;
        }
        session.commit()?;
        Ok::<(), AppError>(())
    })();
    if let Err(error) = result {
        let _ = profile::delete(target, &name);
        return Err(error);
    }

    if matches!(activation::status(target)?, Status::NoFiles) {
        activation::switch(target, &name, false)?;
        style::success(format!("Created and activated profile '{}'", name));
    } else {
        style::success(format!("Created profile '{}'", name));
    }
    Ok(())
}
