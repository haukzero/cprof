mod claude;
mod codex;

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::config;
use crate::error::{AppError, Result};

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
        let is_builtin = BUILTIN_TARGETS
            .iter()
            .any(|builtin| builtin.id == target.id);
        if !is_builtin && command_names.iter().any(|name| name == target.id) {
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
    let path = config::repository_dir()?.join("extra-target.toml");
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let configured =
            toml::from_str::<BTreeMap<String, ExtraTarget>>(&content).map_err(|error| {
                AppError::Other(format!("Failed to parse '{}': {error}", path.display()))
            })?;
        let mut ids = BUILTIN_TARGETS
            .iter()
            .map(|target| target.id)
            .collect::<HashSet<_>>();
        for (name, target) in configured {
            let target = Box::leak(Box::new(target.into_spec(&name)));
            if !ids.insert(target.id) {
                return Err(AppError::TargetConflict(format!(
                    "target id '{}' declared by '{name}' conflicts with an existing target",
                    target.id
                )));
            }
            targets.push(target);
        }
    }
    Ok(targets.into_boxed_slice())
}

#[derive(Debug, Deserialize)]
struct ExtraTarget {
    id: Option<String>,
    #[serde(default)]
    resources: Vec<ExtraResource>,
}

#[derive(Debug, Deserialize)]
struct ExtraResource {
    key: Option<String>,
    filename: String,
    active_path: String,
    template: Option<String>,
    required: Option<bool>,
}

impl ExtraTarget {
    fn into_spec(self, name: &str) -> TargetSpec {
        let id = leak_string(self.id.unwrap_or_else(|| name.to_string()));
        let resources = self
            .resources
            .into_iter()
            .map(ExtraResource::into_spec)
            .collect::<Vec<_>>();
        TargetSpec {
            id,
            resources: Box::leak(resources.into_boxed_slice()),
        }
    }
}

impl ExtraResource {
    fn into_spec(self) -> ResourceSpec {
        let filename = leak_string(self.filename);
        let key = leak_string(self.key.unwrap_or_else(|| filename.to_string()));
        let active_path = leak_string(self.active_path);
        let template = leak_bytes(self.template.unwrap_or_default().into_bytes());
        ResourceSpec {
            key,
            filename,
            active_path,
            required: self.required.unwrap_or(true),
            template,
            validate: external_validate,
        }
    }
}

fn leak_string(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn leak_bytes(value: Vec<u8>) -> &'static [u8] {
    Box::leak(value.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::ExtraTarget;

    #[test]
    fn external_defaults_and_templates_are_loaded() {
        let target: ExtraTarget = toml::from_str(
            r#"
                id = "custom-id"
                [[resources]]
                filename = "settings.conf"
                active_path = ".config/example/settings.conf"
                [[resources]]
                key = "auth"
                filename = "auth.data"
                active_path = ".config/example/auth.data"
                template = "hello"
                required = false
            "#,
        )
        .unwrap();

        let target = target.into_spec("custom");
        assert_eq!(target.id, "custom-id");
        assert_eq!(target.resources[0].key, "settings.conf");
        assert!(target.resources[0].required);
        assert!(target.resources[0].template.is_empty());
        assert_eq!(target.resources[1].key, "auth");
        assert!(!target.resources[1].required);
        assert_eq!(target.resources[1].template, b"hello");
    }
}
