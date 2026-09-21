use std::path::PathBuf;
use std::str::Utf8Error;

/// Home discovery and external target configuration encoding failures.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to parse '{}': {source}", path.display())]
    Encoding {
        path: PathBuf,
        #[source]
        source: Utf8Error,
    },

    #[error("Failed to parse '{}': {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Failed to serialize extra-target.toml: {0}")]
    Serialize(#[source] toml::ser::Error),

    #[error("No home directory found")]
    NoHomeDir,
}
