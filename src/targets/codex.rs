use super::{ResourceSpec, TargetSpec};
use crate::error::{AppError, Result};

fn config_template() -> Vec<u8> {
    b"# Codex configuration\n".to_vec()
}

fn auth_template() -> Vec<u8> {
    b"{}\n".to_vec()
}

fn validate_toml(content: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(content)
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

const RESOURCES: &[ResourceSpec] = &[
    ResourceSpec {
        key: "config",
        filename: "config.toml",
        active_path: ".codex/config.toml",
        required: true,
        template: config_template,
        validate: validate_toml,
    },
    ResourceSpec {
        key: "auth",
        filename: "auth.json",
        active_path: ".codex/auth.json",
        required: true,
        template: auth_template,
        validate: validate_json,
    },
];

pub const SPEC: TargetSpec = TargetSpec {
    id: "codex",
    resources: RESOURCES,
};
