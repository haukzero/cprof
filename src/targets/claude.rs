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

pub(super) fn spec() -> TargetSpec {
    TargetSpec {
        id: "claude".to_string(),
        resources: vec![ResourceSpec {
            key: "settings".to_string(),
            filename: "settings.json".to_string(),
            active_path: Some(".claude/settings.json".to_string()),
            absolute_active_path: None,
            required: true,
            template: TEMPLATE.to_vec(),
            validate,
        }],
    }
}
