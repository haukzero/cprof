use std::str;

use crate::error::{AppError, Result};

use super::{ResourceSpec, TargetSpec};

const CONFIG_TEMPLATE: &[u8] = b"# Codex configuration\n";
const AUTH_TEMPLATE: &[u8] = b"{}\n";

fn validate_toml(content: &[u8]) -> Result<()> {
    let text = str::from_utf8(content)
        .map_err(|_| AppError::InvalidResource("config.toml is not valid UTF-8".to_string()))?;
    toml::from_str::<toml::Value>(text)
        .map(|_| ())
        .map_err(|error| AppError::InvalidResource(format!("invalid TOML: {error}")))
}

fn validate_json(content: &[u8]) -> Result<()> {
    serde_json::from_slice::<serde_json::Value>(content)
        .map(|_| ())
        .map_err(AppError::Json)
}

pub(super) fn spec() -> TargetSpec {
    TargetSpec {
        id: "codex".to_string(),
        resources: vec![
            ResourceSpec {
                key: "config".to_string(),
                filename: "config.toml".to_string(),
                active_path: Some(".codex/config.toml".to_string()),
                absolute_active_path: None,
                required: true,
                template: CONFIG_TEMPLATE.to_vec(),
                validate: validate_toml,
            },
            ResourceSpec {
                key: "auth".to_string(),
                filename: "auth.json".to_string(),
                active_path: Some(".codex/auth.json".to_string()),
                absolute_active_path: None,
                required: true,
                template: AUTH_TEMPLATE.to_vec(),
                validate: validate_json,
            },
        ],
    }
}
