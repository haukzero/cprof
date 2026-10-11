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

    #[error("Edit was not committed; original file was kept: {source}")]
    NotCommitted { source: Box<AppError> },
}
