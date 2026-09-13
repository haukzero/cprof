#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Profile '{0}' already exists")]
    ProfileExists(String),

    #[error("Profile '{0}' not found")]
    ProfileNotFound(String),

    #[error("No profiles matched '{0}'")]
    NoProfilesMatched(String),

    #[error("Invalid profile name '{0}'")]
    InvalidProfileName(String),

    #[error("Unknown target '{0}'")]
    UnknownTarget(String),

    #[error("Target conflict: {0}")]
    TargetConflict(String),

    #[error("Unknown resource '{resource}' for target '{target}'")]
    UnknownResource { target: String, resource: String },

    #[error("Profile '{0}' is incomplete")]
    IncompleteProfile(String),

    #[error("Active path '{0}' is not managed by cprof; use --force to replace it")]
    UnmanagedActivePath(String),

    #[error("Invalid resource: {0}")]
    InvalidResource(String),

    #[error("Editor '{0}' not found or failed to launch")]
    EditorNotFound(String),

    #[error("Editor exited with non-zero status")]
    EditorFailed,

    #[error("UAC elevation was cancelled or failed")]
    ElevationFailed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid package file: {0}")]
    InvalidPackage(String),

    #[error("Package checksum mismatch - file may be corrupted or tampered")]
    ChecksumMismatch,

    #[error("No home directory found")]
    NoHomeDir,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
