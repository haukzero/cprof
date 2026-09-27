use super::AppError;

/// Transaction conflicts and failures during rollback or committed cleanup.
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
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

fn format_errors(errors: &[AppError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
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
