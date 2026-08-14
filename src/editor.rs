use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config;
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

/// Create a temporary directory and file for editing
pub fn create_temp_file(name: &str, initial_content: &str) -> Result<(PathBuf, PathBuf)> {
    let temp_dir = config::ensure_profiles_dir()?.join(".tmp");
    fs::create_dir_all(&temp_dir)?;
    let temp_file = temp_dir.join(format!("{}.json", name));
    fs::write(&temp_file, initial_content)?;
    Ok((temp_dir, temp_file))
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

/// Clean up temporary directory and file
pub fn cleanup_temp(temp_dir: &Path, temp_file: &Path) {
    let _ = fs::remove_file(temp_file);
    let _ = fs::remove_dir(temp_dir);
}

/// Edit content in an editor and return the new content
///
/// This handles the full flow:
/// 1. Create temp file with initial content
/// 2. Open editor
/// 3. Read new content
/// 4. Clean up temp files
pub fn edit_content(name: &str, initial_content: &str, editor: Option<String>) -> Result<String> {
    let editor_cmd = resolve_editor(editor)?;
    let (temp_dir, temp_file) = create_temp_file(name, initial_content)?;

    // Open editor
    if let Err(e) = open_editor(&editor_cmd, &temp_file) {
        cleanup_temp(&temp_dir, &temp_file);
        return Err(e);
    }

    // Read the edited content
    let new_content = fs::read_to_string(&temp_file)?;

    // Clean up
    cleanup_temp(&temp_dir, &temp_file);

    Ok(new_content)
}
