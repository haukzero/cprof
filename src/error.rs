use std::path::{Path, PathBuf};

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

    #[error("Invalid target id '{0}'")]
    InvalidTargetId(String),

    #[error("Invalid filename '{0}'")]
    InvalidFilename(String),

    #[error("Invalid active path '{0}'")]
    InvalidActivePath(String),

    #[error("Unsafe path '{0}'")]
    UnsafePath(String),

    #[error("Unknown target '{0}'")]
    UnknownTarget(String),

    #[error("Target conflict: {0}")]
    TargetConflict(String),

    #[error("Transaction conflict: {0}")]
    TransactionConflict(String),

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

    #[error("Edit was not committed; original file was kept: {source}")]
    EditNotCommitted { source: Box<AppError> },

    #[error("UAC elevation was cancelled or failed")]
    ElevationFailed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("IO error for '{}': {source}", path.display())]
    IoPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

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

impl AppError {
    pub(crate) fn io(path: &Path, source: std::io::Error) -> Self {
        Self::IoPath {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn is_io_kind(&self, kind: std::io::ErrorKind) -> bool {
        self.io_source().is_some_and(|source| source.kind() == kind)
    }

    pub(crate) fn io_source(&self) -> Option<&std::io::Error> {
        match self {
            Self::Io(source) | Self::IoPath { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub(crate) trait IoContext<T> {
    fn with_path(self, path: &Path) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn with_path(self, path: &Path) -> Result<T> {
        self.map_err(|source| AppError::io(path, source))
    }
}
