use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::error::{AppError, IoContext, Result};
use crate::fs_util::PathTransaction;

pub struct EditSession {
    editor: String,
    editor_args: Vec<String>,
    transaction: Option<PathTransaction>,
    changed: bool,
}

impl EditSession {
    pub fn new(editor: Option<String>, editor_args: Vec<String>) -> Result<Self> {
        let (editor, editor_args) = match editor {
            Some(editor) => (editor, editor_args),
            None => match ["VISUAL", "EDITOR"].into_iter().find_map(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            }) {
                Some(command) => {
                    let mut words = shell_words::split(&command)
                        .map_err(|error| AppError::InvalidEditorCommand(error.to_string()))?
                        .into_iter();
                    let editor = words.next().ok_or_else(|| {
                        AppError::InvalidEditorCommand("command is empty".to_string())
                    })?;
                    (editor, words.collect())
                }
                None => (
                    if cfg!(windows) { "notepad" } else { "vi" }.to_string(),
                    Vec::new(),
                ),
            },
        };
        Ok(Self {
            editor,
            editor_args,
            transaction: Some(PathTransaction::new()),
            changed: false,
        })
    }

    pub fn edit(&mut self, path: &Path, validate: impl FnOnce(&[u8]) -> Result<()>) -> Result<()> {
        let before = match fs::read(path) {
            Ok(content) => content,
            Err(error) => return Err(self.abort(AppError::io(path, error))),
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
            Err(error) => return Err(self.abort(AppError::io(path, error))),
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
        let status = Command::new(&self.editor)
            .args(&self.editor_args)
            .arg(&draft)
            .status()
            .map_err(|_| AppError::EditorNotFound(self.editor.clone()))?;
        if !status.success() {
            return Err(AppError::EditorFailed);
        }
        let after = fs::read(&draft).with_path(&draft)?;
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
