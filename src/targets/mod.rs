mod claude;
mod codex;
mod external;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::{AppError, Result};

pub(crate) use external::{
    ExternalResourceConfig, ExternalTargetConfig, external_config_for, find_external_config,
    merge_external_configs, read_external_configs, spec_from_external_config,
    write_external_configs,
};

pub type Validator = fn(&[u8]) -> Result<()>;

#[derive(Debug, Clone, Copy)]
pub struct ResourceSpec {
    pub key: &'static str,
    pub filename: &'static str,
    pub active_path: &'static str,
    pub required: bool,
    pub template: &'static [u8],
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

fn external_validate(_: &[u8]) -> Result<()> {
    Ok(())
}

const BUILTIN_TARGETS: &[&TargetSpec] = &[&claude::SPEC, &codex::SPEC];

pub fn get(id: &str) -> Result<&'static TargetSpec> {
    all()?
        .iter()
        .copied()
        .find(|target| target.id == id)
        .ok_or_else(|| AppError::UnknownTarget(id.to_string()))
}

pub fn all() -> Result<&'static [&'static TargetSpec]> {
    static TARGETS: OnceLock<std::result::Result<Box<[&'static TargetSpec]>, AppError>> =
        OnceLock::new();
    match TARGETS.get_or_init(load_from_config) {
        Ok(targets) => Ok(targets),
        Err(error) => Err(AppError::Other(error.to_string())),
    }
}

pub fn validate_command_conflicts(command_names: &[String]) -> Result<()> {
    for target in all()? {
        if !is_builtin(target) && command_names.iter().any(|name| name == target.id) {
            return Err(AppError::TargetConflict(format!(
                "target id '{}' conflicts with the command of the same name",
                target.id
            )));
        }
    }
    Ok(())
}

fn load_from_config() -> Result<Box<[&'static TargetSpec]>> {
    let mut targets = BUILTIN_TARGETS.to_vec();
    let configured = read_external_configs()?;
    let mut ids = HashSet::new();
    for (name, target_config) in configured {
        let target = Box::leak(Box::new(spec_from_external_config(&target_config)?));
        if !ids.insert(target.id) {
            return Err(AppError::TargetConflict(format!(
                "target id '{}' declared by '{name}' conflicts with an existing target",
                target.id,
            )));
        }
        targets.push(target);
    }
    Ok(targets.into_boxed_slice())
}

pub(crate) fn is_builtin(target: &TargetSpec) -> bool {
    BUILTIN_TARGETS
        .iter()
        .any(|builtin| builtin.id == target.id)
}

#[cfg(test)]
mod tests {
    use super::is_builtin;

    #[test]
    fn builtin_targets_are_identified() {
        assert!(is_builtin(super::get("claude").unwrap()));
        assert!(is_builtin(super::get("codex").unwrap()));
    }
}
