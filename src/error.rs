use std::io;
use std::path::{Path, PathBuf};
use std::result;
use std::str::Utf8Error;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // NOTE: Profile lifecycle
    #[error("Profile '{0}' already exists")]
    ProfileExists(String),

    #[error("Profile '{0}' not found")]
    ProfileNotFound(String),

    #[error("No profiles matched '{0}'")]
    NoProfilesMatched(String),

    #[error("Invalid profile name '{0}'")]
    InvalidProfileName(String),

    #[error("Profile '{0}' is incomplete")]
    IncompleteProfile(String),

    // NOTE: Targets and resources
    #[error("Invalid target id '{0}'")]
    InvalidTargetId(String),

    #[error("Unknown target '{0}'")]
    UnknownTarget(String),

    #[error("Target conflict: {0}")]
    TargetConflict(String),

    #[error("No targets selected")]
    NoTargetsSelected,

    #[error("Unknown resource '{resource}' for target '{target}'")]
    UnknownResource { target: String, resource: String },

    #[error("Invalid resource: {0}")]
    InvalidResource(String),

    // NOTE: Paths and activation
    #[error("Invalid filename '{0}'")]
    InvalidFilename(String),

    #[error("Invalid active path '{0}'")]
    InvalidActivePath(String),

    #[error("Unsafe path '{0}'")]
    UnsafePath(String),

    #[error("Active path '{0}' is not managed by cprof; use --force to replace it")]
    UnmanagedActivePath(String),

    #[error(
        "Target '{target}' is already managed by profile '{profile}'; use create --copy-from or rename"
    )]
    AlreadyManaged { target: String, profile: String },

    #[error("No current configuration to adopt for '{0}'")]
    NoActiveConfiguration(String),

    #[error(
        "Target '{0}' has incomplete or mixed managed links; use switch to restore a complete profile"
    )]
    InconsistentManagedLinks(String),

    #[error("Current configuration at '{}' changed while preparing adoption; retry the command", .0.display())]
    ActiveConfigurationChanged(PathBuf),

    #[error("Required resource '{resource}' is missing at '{}'", path.display())]
    MissingActiveResource { resource: String, path: PathBuf },

    #[error("Invalid resource at '{}': {source}", path.display())]
    ResourceValidation {
        path: PathBuf,
        #[source]
        source: Box<AppError>,
    },

    #[error("Failed to parse '{}': {source}", path.display())]
    ExternalConfigEncoding {
        path: PathBuf,
        #[source]
        source: Utf8Error,
    },

    #[error("Failed to parse '{}': {source}", path.display())]
    ExternalConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Failed to serialize extra-target.toml: {0}")]
    ExternalConfigSerialize(#[source] toml::ser::Error),

    // NOTE: Editor
    #[error("Editor '{0}' not found or failed to launch")]
    EditorNotFound(String),

    #[error("Invalid editor command: {0}")]
    InvalidEditorCommand(String),

    #[error("Editor exited with non-zero status")]
    EditorFailed,

    #[error("Edit was not committed; original file was kept: {source}")]
    EditNotCommitted { source: Box<AppError> },

    // NOTE: Packages
    #[error("Invalid package file: {0}")]
    InvalidPackage(String),

    #[error("Package file '{0}' not found")]
    PackageNotFound(PathBuf),

    #[error("Package checksum mismatch - file may be corrupted or tampered")]
    ChecksumMismatch,

    // NOTE: Runtime and infrastructure
    #[error("Transaction conflict: {0}")]
    TransactionConflict(String),

    #[error("Transaction committed, but cleanup failed: {}", format_errors(.0))]
    TransactionCleanupFailed(Vec<AppError>),

    #[error("{source}; rollback or cleanup also failed: {}", format_errors(.errors))]
    RecoveryFailed {
        #[source]
        source: Box<AppError>,
        errors: Vec<AppError>,
    },

    #[error("UAC elevation was cancelled or failed")]
    ElevationFailed,

    #[error("Missing original home directory after --elevated-home")]
    MissingElevatedHome,

    #[error("Elevated home was already initialized")]
    ElevationAlreadyInitialized,

    #[error("Interactive input required but no interactive terminal is available")]
    InteractiveInputRequired,

    #[error("Interactive prompt failed: {0}")]
    Prompt(#[from] dialoguer::Error),

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

    #[error("No home directory found")]
    NoHomeDir,
}

pub type Result<T> = result::Result<T, AppError>;

impl AppError {
    /// Preserve the original error when recovery succeeds. A failed recovery
    /// remains distinct so callers cannot mistake it for a safe retry.
    pub(crate) fn with_recovery(self, errors: impl IntoIterator<Item = AppError>) -> Self {
        let errors = errors.into_iter().collect::<Vec<_>>();
        if errors.is_empty() {
            self
        } else {
            Self::RecoveryFailed {
                source: Box::new(self),
                errors,
            }
        }
    }

    pub(crate) fn io(path: &Path, source: io::Error) -> Self {
        Self::IoPath {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn is_io_kind(&self, kind: io::ErrorKind) -> bool {
        self.io_source().is_some_and(|source| source.kind() == kind)
    }

    pub(crate) fn io_source(&self) -> Option<&io::Error> {
        // Do not unwrap RecoveryFailed or TransactionCleanupFailed here:
        // retrying those operations could overwrite unrecovered data.
        match self {
            Self::Io(source) | Self::IoPath { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn format_errors(errors: &[AppError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) trait IoContext<T> {
    fn with_path(self, path: &Path) -> Result<T>;
}

impl<T> IoContext<T> for io::Result<T> {
    fn with_path(self, path: &Path) -> Result<T> {
        self.map_err(|source| AppError::io(path, source))
    }
}
