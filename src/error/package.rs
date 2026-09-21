use std::path::PathBuf;

/// Package discovery, format, and integrity failures.
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("Invalid package file: {0}")]
    Invalid(String),

    #[error("Package file '{0}' not found")]
    NotFound(PathBuf),

    #[error("Package checksum mismatch - file may be corrupted or tampered")]
    ChecksumMismatch,
}
