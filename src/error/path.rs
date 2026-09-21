/// Path syntax and filesystem containment violations.
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("Invalid filename '{0}'")]
    InvalidFilename(String),

    #[error("Invalid active path '{0}'")]
    InvalidActivePath(String),

    #[error("Unsafe path '{0}'")]
    Unsafe(String),
}
