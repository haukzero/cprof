#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("Invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("Invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
}

impl From<serde_json::Error> for super::AppError {
    fn from(source: serde_json::Error) -> Self {
        FormatError::from(source).into()
    }
}
