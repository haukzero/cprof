use super::{ResourceSpec, TargetSpec};
use crate::error::{AppError, Result};

fn template() -> Vec<u8> {
    br#"{
  "env": {},
  "permissions": {
    "allow": []
  }
}
"#
    .to_vec()
}

fn validate(content: &[u8]) -> Result<()> {
    serde_json::from_slice::<serde_json::Value>(content)
        .map(|_| ())
        .map_err(AppError::Json)
}

const RESOURCES: &[ResourceSpec] = &[ResourceSpec {
    key: "settings",
    filename: "settings.json",
    active_path: ".claude/settings.json",
    required: true,
    template,
    validate,
}];

pub const SPEC: TargetSpec = TargetSpec {
    id: "claude",
    resources: RESOURCES,
};
