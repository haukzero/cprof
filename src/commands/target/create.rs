use crate::cli::EditorOptions;
use crate::config;
use crate::elevate;
use crate::error::Result;
use crate::profile::activation::{self, Status};
use crate::profile::storage;
use crate::targets::TargetSpec;
use crate::ui::{editor::EditSession, prompt, style};

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
    // Keep the lock across profile creation so an editor cannot observe a
    // newly-created profile before its initial transaction is ready.
    let mut session = EditSession::new(
        editor.editor,
        editor.editor_args,
        &config::profile_dir(target, &name)?,
    )?;
    let recovering = storage::has_edit_drafts(target, &name)?;
    if !recovering {
        storage::create(target, &name, copy_from.as_deref())?;
    }
    let result = (|| {
        for resource in &target.resources {
            let path = storage::resource_path(target, &name, resource)?;
            session.edit(&path, |bytes| resource.validate(bytes))?;
        }
        session.commit()
    })();
    if let Err(error) = result {
        if !recovering {
            let _ = storage::delete(target, &name);
        }
        return Err(error);
    }

    if matches!(activation::status(target)?, Status::NoFiles) {
        elevate::run(
            &[
                target.id.clone().into(),
                "switch".into(),
                name.clone().into(),
            ],
            || activation::switch(target, &name, false),
        )?;
        style::success(format!("Created and activated profile '{}'", name));
    } else {
        style::success(format!("Created profile '{}'", name));
    }
    Ok(())
}
