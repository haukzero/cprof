use crate::format::Format;
use crate::targets::{ResourceSpec, TargetSpec};

const CONFIG_TEMPLATE: &[u8] = b"# Codex configuration\n";
const AUTH_TEMPLATE: &[u8] = b"{}\n";

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
                format: Format::Toml,
            },
            ResourceSpec {
                key: "auth".to_string(),
                filename: "auth.json".to_string(),
                active_path: Some(".codex/auth.json".to_string()),
                absolute_active_path: None,
                required: true,
                template: AUTH_TEMPLATE.to_vec(),
                format: Format::Json,
            },
        ],
    }
}
