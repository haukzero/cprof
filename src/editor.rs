use std::path::Path;
use std::process::Command;

use crate::error::{AppError, Result};

pub fn resolve_editor(editor: Option<String>) -> Result<String> {
    match editor {
        Some(editor) => {
            if which::which(&editor).is_err() {
                return Err(AppError::EditorNotFound(editor));
            }
            Ok(editor)
        }
        None => Ok(default_editor().unwrap_or_else(|| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        })),
    }
}

fn default_editor() -> Option<String> {
    std::env::var("EDITOR")
        .ok()
        .or_else(|| std::env::var("VISUAL").ok())
}

pub fn open_editor(editor: &str, file: &Path) -> Result<()> {
    let status = Command::new(editor)
        .arg(file)
        .status()
        .map_err(|_| AppError::EditorNotFound(editor.to_string()))?;
    if !status.success() {
        return Err(AppError::EditorFailed);
    }
    Ok(())
}
