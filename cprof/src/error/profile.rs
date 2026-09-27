/// Profile naming, selection, and lifecycle failures.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("Profile '{0}' already exists")]
    Exists(String),

    #[error("Profile '{0}' not found")]
    NotFound(String),

    #[error("No profiles matched '{0}'")]
    NoMatches(String),

    #[error("Invalid profile name '{0}'")]
    InvalidName(String),

    #[error("Profile '{0}' is incomplete")]
    Incomplete(String),
}
