use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::error::{AppError, Result};
use crate::fs_util::PathTransaction;

pub struct EditSession {
    editor: String,
    transaction: Option<PathTransaction>,
    changed: bool,
}

impl EditSession {
    pub fn new(editor: Option<String>) -> Result<Self> {
        Ok(Self {
            editor: resolve_editor(editor)?,
            transaction: Some(PathTransaction::new()),
            changed: false,
        })
    }

    pub fn edit(&mut self, path: &Path, validate: impl FnOnce(&[u8]) -> Result<()>) -> Result<()> {
        let before = match fs::read(path) {
            Ok(content) => content,
            Err(error) => return Err(self.abort(error.into())),
        };
        self.edit_content(path, before, true, validate)
    }

    pub fn edit_or_create(
        &mut self,
        path: &Path,
        default: &[u8],
        validate: impl FnOnce(&[u8]) -> Result<()>,
    ) -> Result<()> {
        let (before, replace) = match fs::read(path) {
            Ok(content) => (content, true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (default.to_vec(), false),
            Err(error) => return Err(self.abort(error.into())),
        };
        self.edit_content(path, before, replace, validate)
    }

    fn edit_content(
        &mut self,
        path: &Path,
        before: Vec<u8>,
        replace: bool,
        validate: impl FnOnce(&[u8]) -> Result<()>,
    ) -> Result<()> {
        match self.edit_inner(path, &before, replace, validate) {
            Ok(changed) => {
                self.changed |= changed;
                Ok(())
            }
            Err(error) => Err(self.abort(error)),
        }
    }

    pub fn commit(mut self) -> Result<bool> {
        self.transaction
            .take()
            .expect("edit transaction is active")
            .commit()?;
        Ok(self.changed)
    }

    fn edit_inner(
        &mut self,
        path: &Path,
        before: &[u8],
        replace: bool,
        validate: impl FnOnce(&[u8]) -> Result<()>,
    ) -> Result<bool> {
        let draft = self
            .transaction
            .as_mut()
            .expect("edit transaction is active")
            .stage_file(path, "edit", before, replace)?;
        open_editor(&self.editor, &draft)?;
        let after = fs::read(&draft)?;
        if replace && after == before {
            self.transaction
                .as_mut()
                .expect("edit transaction is active")
                .discard_last()?;
            return Ok(false);
        }
        validate(&after).map_err(|source| AppError::EditNotCommitted {
            source: Box::new(source),
        })?;
        Ok(true)
    }

    fn abort(&mut self, error: AppError) -> AppError {
        match self.transaction.take() {
            Some(transaction) => transaction.cancel(error),
            None => error,
        }
    }
}

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
