mod json;
mod toml;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Toml,
    Any,
}

impl Format {
    pub fn check(self, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Json => json::validate(bytes),
            Self::Toml => toml::validate(bytes),
            Self::Any => Ok(()),
        }
        .map_err(AppError::from)
    }
}
