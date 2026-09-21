/// Privilege elevation and propagation of the original home directory.
#[derive(Debug, thiserror::Error)]
pub enum ElevationError {
    #[error("UAC elevation was cancelled or failed")]
    Failed,

    #[error("Missing original home directory after --elevated-home")]
    MissingHome,

    #[error("Elevated home was already initialized")]
    AlreadyInitialized,
}
