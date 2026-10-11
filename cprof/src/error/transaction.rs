use std::path::PathBuf;

use super::AppError;
use super::support::{display_path, format_errors};

/// Transaction conflicts and failures during rollback or committed cleanup.
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error(
        "Cannot modify '{}': another cprof operation is active. Close it and retry. If the terminal was interrupted, stop the process holding the lock (Unix: lsof/fuser; Windows: Task Manager). Existing draft will be reused when editing resumes. Lock: '{}'",
        display_path(.scope),
        display_path(.lock)
    )]
    Locked { scope: PathBuf, lock: PathBuf },

    #[error("Transaction conflict: {0}")]
    Conflict(String),

    #[error("Transaction committed, but cleanup failed: {}", format_errors(.0))]
    CleanupFailed(Vec<AppError>),

    #[error("{source}; rollback or cleanup also failed: {}", format_errors(.errors))]
    RecoveryFailed {
        #[source]
        source: Box<AppError>,
        errors: Vec<AppError>,
    },
}

impl AppError {
    /// Preserve the original error when recovery succeeds. A failed recovery
    /// remains distinct so callers cannot mistake it for a safe retry.
    pub(crate) fn with_recovery(self, errors: impl IntoIterator<Item = AppError>) -> Self {
        let errors = errors.into_iter().collect::<Vec<_>>();
        if errors.is_empty() {
            self
        } else {
            TransactionError::RecoveryFailed {
                source: Box::new(self),
                errors,
            }
            .into()
        }
    }
}
