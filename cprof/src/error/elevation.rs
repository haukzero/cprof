/// Privilege elevation and propagation of retry state.
#[derive(Debug, thiserror::Error)]
pub enum ElevationError {
    #[error("UAC elevation was cancelled or failed")]
    Failed,

    #[error("Missing original home directory after --elevated-home")]
    MissingHome,

    #[error("Elevated home was already initialized")]
    AlreadyInitialized,

    #[error("Invalid elevation retry context: {0}")]
    InvalidContext(String),

    #[error(
        "The retry context no longer matches the current operation; run the command again in the original terminal"
    )]
    ReplayMismatch,
}
