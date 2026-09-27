use crate::format::Format;
use crate::targets::{ResourceSpec, TargetSpec};

const TEMPLATE: &[u8] = br#"{
  "env": {},
  "permissions": {
    "allow": []
  }
}
"#;

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
            format: Format::Json,
        }],
    }
}
