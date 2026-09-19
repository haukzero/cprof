use super::{ResourceSpec, TargetSpec};
use crate::error::{AppError, Result};

const TEMPLATE: &[u8] = br#"{
  "env": {},
  "permissions": {
    "allow": []
  }
}
"#;

fn validate(content: &[u8]) -> Result<()> {
    serde_json::from_slice::<serde_json::Value>(content)
        .map(|_| ())
        .map_err(AppError::Json)
}

const RESOURCES: &[ResourceSpec] = &[ResourceSpec {
    key: "settings",
    filename: "settings.json",
    active_path: Some(".claude/settings.json"),
    absolute_active_path: None,
    required: true,
    template: TEMPLATE,
    validate,
}];

pub(super) const SPEC: TargetSpec = TargetSpec {
    id: "claude",
    resources: RESOURCES,
};
