use crate::cli::EditorOptions;
use crate::error::Result;
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;
use crate::ui::editor::EditSession;

use super::write::WriteSession;

/// Keep creation, initial editing, and activation in the same write session.
/// Only activation is retried with elevation, so preparation is a separate step.
pub(crate) struct CreatedProfile<'a> {
    target: &'a TargetSpec,
    name: &'a str,
    write: WriteSession,
}

pub(crate) fn prepare<'a>(
    target: &'a TargetSpec,
    name: &'a str,
    copy_from: Option<&str>,
    editor: EditorOptions,
) -> Result<CreatedProfile<'a>> {
    let write = WriteSession::targets([target.id.as_str()])?;
    let mut session = EditSession::new(editor.editor, editor.editor_args)?;
    let recovering = storage::has_edit_drafts(target, name)?;
    if !recovering {
        storage::create(target, name, copy_from)?;
    }
    let result = (|| {
        for resource in &target.resources {
            let path = storage::resource_path(target, name, resource)?;
            session.edit(&path, |bytes| resource.validate(bytes))?;
        }
        session.commit()
    })();
    if let Err(error) = result {
        if !recovering {
            let _ = storage::delete(target, name);
        }
        return Err(error);
    }
    Ok(CreatedProfile {
        target,
        name,
        write,
    })
}

impl CreatedProfile<'_> {
    pub(crate) fn activate(self) -> Result<bool> {
        self.write.run(|| {
            if !matches!(
                activation::status(self.target)?,
                activation::Status::NoFiles
            ) {
                return Ok(false);
            }
            activation::switch(self.target, self.name, false)?;
            Ok(true)
        })
    }
}
