use std::path::PathBuf;

/// Active configuration, managed links, and adoption failures.
#[derive(Debug, thiserror::Error)]
pub enum ActivationError {
    #[error("Active path '{0}' is not managed by cprof; use --force to replace it")]
    UnmanagedPath(String),

    #[error(
        "Target '{target}' is already managed by profile '{profile}'; use create --copy-from or rename"
    )]
    AlreadyManaged { target: String, profile: String },

    #[error("No current configuration to adopt for '{0}'")]
    NoConfiguration(String),

    #[error(
        "Target '{0}' has incomplete or mixed managed links; use switch to restore a complete profile"
    )]
    InconsistentLinks(String),

    #[error("Current configuration at '{}' changed while preparing adoption; retry the command", .0.display())]
    ConfigurationChanged(PathBuf),

    #[error("Required resource '{resource}' is missing at '{}'", path.display())]
    MissingResource { resource: String, path: PathBuf },
}
