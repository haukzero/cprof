use std::path::PathBuf;

use super::AppError;

/// Editor execution and validation of pending edits.
#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("Editor '{0}' not found or failed to launch")]
    NotFound(String),

    #[error("Invalid editor command: {0}")]
    InvalidCommand(String),

    #[error("Editor exited with non-zero status")]
    Failed,

    #[error(
        "Cannot edit '{}': another cprof edit session is active. Close it and retry. If the terminal was interrupted, stop the process holding the lock (Unix: lsof/fuser; Windows: Task Manager). Existing draft will be reused. Lock: '{}'",
        scope.display(),
        lock.display()
    )]
    Locked { scope: PathBuf, lock: PathBuf },

    #[error("Edit was not committed; original file was kept: {source}")]
    NotCommitted { source: Box<AppError> },
}
