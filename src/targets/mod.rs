mod claude;
mod codex;

use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};

pub type TemplateFactory = fn() -> Vec<u8>;
pub type Validator = fn(&[u8]) -> Result<()>;

#[derive(Debug, Clone, Copy)]
pub struct ResourceSpec {
    pub key: &'static str,
    pub filename: &'static str,
    pub active_path: &'static str,
    pub required: bool,
    pub template: TemplateFactory,
    pub validate: Validator,
}

#[derive(Debug)]
pub struct TargetSpec {
    pub id: &'static str,
    pub resources: &'static [ResourceSpec],
}

impl TargetSpec {
    pub fn resource(&self, key: &str) -> Result<&'static ResourceSpec> {
        self.resources
            .iter()
            .find(|resource| resource.key == key)
            .ok_or_else(|| AppError::UnknownResource {
                target: self.id.to_string(),
                resource: key.to_string(),
            })
    }

    pub fn active_path(&self, home: &Path, resource: &ResourceSpec) -> PathBuf {
        home.join(resource.active_path)
    }
}

pub fn get(id: &str) -> Result<&'static TargetSpec> {
    match id {
        "claude" => Ok(&claude::SPEC),
        "codex" => Ok(&codex::SPEC),
        _ => Err(AppError::UnknownTarget(id.to_string())),
    }
}

pub fn all() -> &'static [&'static TargetSpec] {
    static TARGETS: &[&TargetSpec] = &[&claude::SPEC, &codex::SPEC];
    TARGETS
}
