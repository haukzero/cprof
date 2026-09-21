//! Typed errors grouped by domain, with a shared application boundary.
//!
//! Construct domain errors where failures originate and convert with `?` or
//! `.into()`. Transparent wrappers preserve their messages and underlying causes.
//! Match the category and variant when handling a specific failure, for example
//! `AppError::Profile(ProfileError::NotFound(name))`.
//! Add new failures to their domain enum; only new domains need an AppError variant.

use std::io;
use std::path::PathBuf;

mod activation;
mod config;
mod editor;
mod elevation;
mod interaction;
mod io_context;
mod package;
mod path;
mod profile;
mod target;
mod transaction;

#[cfg(test)]
mod tests;

pub use activation::ActivationError;
pub use config::ConfigError;
pub use editor::EditorError;
pub use elevation::ElevationError;
pub use interaction::InteractionError;
pub use package::PackageError;
pub use path::PathError;
pub use profile::ProfileError;
pub use target::TargetError;
pub use transaction::TransactionError;

pub(crate) use io_context::IoContext;

/// Shared error boundary for operations spanning multiple domains.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Profile(#[from] ProfileError),

    #[error(transparent)]
    Target(#[from] TargetError),

    #[error(transparent)]
    Path(#[from] PathError),

    #[error(transparent)]
    Activation(#[from] ActivationError),

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Editor(#[from] EditorError),

    #[error(transparent)]
    Package(#[from] PackageError),

    #[error(transparent)]
    Transaction(#[from] TransactionError),

    #[error(transparent)]
    Elevation(#[from] ElevationError),

    #[error(transparent)]
    Interaction(#[from] InteractionError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("IO error for '{}': {source}", path.display())]
    IoPath {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;
