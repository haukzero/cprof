use std::path::PathBuf;

use super::AppError;

/// Target definitions, resource lookup, and resource validation failures.
#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("Invalid target id '{0}'")]
    InvalidId(String),

    #[error("Unknown target '{0}'")]
    Unknown(String),

    #[error("Target conflict: {0}")]
    Conflict(String),

    #[error("No targets selected")]
    NoneSelected,

    #[error("Unknown resource '{resource}' for target '{target}'")]
    UnknownResource { target: String, resource: String },

    #[error("Invalid resource: {0}")]
    InvalidResource(String),

    #[error("Invalid resource at '{}': {source}", path.display())]
    ResourceValidation {
        path: PathBuf,
        #[source]
        source: Box<AppError>,
    },
}
