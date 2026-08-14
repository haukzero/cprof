#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Profile '{0}' already exists")]
    ProfileExists(String),

    #[error("Profile '{0}' not found")]
    ProfileNotFound(String),

    #[error("No active profile")]
    #[allow(dead_code)]
    NoActiveProfile,

    #[error("Editor '{0}' not found or failed to launch")]
    EditorNotFound(String),

    #[error("Editor exited with non-zero status")]
    EditorFailed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid package file: {0}")]
    InvalidPackage(String),

    #[error("Package checksum mismatch - file may be corrupted or tampered")]
    ChecksumMismatch,

    #[error("Package conflict: profile '{0}' already exists")]
    #[allow(dead_code)]
    PackageConflict(String),

    #[error("No home directory found")]
    NoHomeDir,

    #[error("settings.json exists but is not a symlink managed by cprof")]
    NotASymlink,

    #[error("settings.json symlink points to an external path: {0}")]
    ExternalSymlink(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
