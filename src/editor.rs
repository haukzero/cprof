use std::path::Path;
use std::process::Command;

use crate::error::{AppError, Result};
use crate::profile;

/// Resolve the editor command to use
pub fn resolve_editor(editor: Option<String>) -> Result<String> {
    match editor {
        Some(e) => {
            if !profile::command_exists(&e) {
                return Err(AppError::EditorNotFound(e));
            }
            Ok(e)
        }
        None => Ok(profile::default_editor().unwrap_or_else(|| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        })),
    }
}

/// Open an editor for the given file
pub fn open_editor(editor_cmd: &str, file: &Path) -> Result<()> {
    let status = Command::new(editor_cmd)
        .arg(file)
        .status()
        .map_err(|_| AppError::EditorNotFound(editor_cmd.to_string()))?;

    if !status.success() {
        return Err(AppError::EditorFailed);
    }

    Ok(())
}
