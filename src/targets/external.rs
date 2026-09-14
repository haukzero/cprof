use std::collections::{BTreeMap, HashSet};
use std::fs;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::{AppError, Result};

use super::{EXTRA_TARGET_FILE, ResourceSpec, TargetSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalTargetConfig {
    pub(crate) name: String,
    pub(crate) id: Option<String>,
    pub(crate) resources: Vec<ExternalResourceConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalResourceConfig {
    pub(crate) key: Option<String>,
    pub(crate) filename: String,
    pub(crate) active_path: String,
    pub(crate) template: Option<String>,
    pub(crate) required: Option<bool>,
}

impl ExternalTargetConfig {
    pub(crate) fn resolved_id(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.name)
    }

    pub(crate) fn compact(mut self) -> Self {
        if self.id.as_deref() == Some(self.name.as_str()) {
            self.id = None;
        }
        for resource in &mut self.resources {
            if resource.key.as_deref() == Some(resource.filename.as_str()) {
                resource.key = None;
            }
            if resource.template.as_deref() == Some("") {
                resource.template = None;
            }
            if resource.required == Some(true) {
                resource.required = None;
            }
        }
        self
    }
}

impl ExternalResourceConfig {
    fn resolved_key(&self) -> &str {
        self.key.as_deref().unwrap_or(&self.filename)
    }

    fn same_definition(&self, other: &Self) -> bool {
        self.resolved_key() == other.resolved_key()
            && self.filename == other.filename
            && self.active_path == other.active_path
            && self.template.as_deref().unwrap_or("") == other.template.as_deref().unwrap_or("")
            && self.required.unwrap_or(true) == other.required.unwrap_or(true)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ExtraTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    resources: Vec<ExtraResource>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ExtraResource {
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    filename: String,
    active_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<bool>,
}

impl ExtraTarget {
    fn into_config(self, name: String) -> ExternalTargetConfig {
        ExternalTargetConfig {
            name,
            id: self.id,
            resources: self
                .resources
                .into_iter()
                .map(|resource| ExternalResourceConfig {
                    key: resource.key,
                    filename: resource.filename,
                    active_path: resource.active_path,
                    template: resource.template,
                    required: resource.required,
                })
                .collect(),
        }
    }
}

impl ExternalResourceConfig {
    fn to_spec(&self) -> ResourceSpec {
        let filename = leak_string(self.filename.clone());
        let key = leak_string(self.key.clone().unwrap_or_else(|| self.filename.clone()));
        let active_path = leak_string(self.active_path.clone());
        let template = leak_bytes(self.template.clone().unwrap_or_default().into_bytes());
        ResourceSpec {
            key,
            filename,
            active_path,
            required: self.required.unwrap_or(true),
            template,
            validate: super::external_validate,
        }
    }
}

pub(crate) fn read_external_configs() -> Result<BTreeMap<String, ExternalTargetConfig>> {
    let path = config::repository_dir()?.join(EXTRA_TARGET_FILE);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let content = fs::read_to_string(&path)?;
    let configured =
        toml::from_str::<BTreeMap<String, ExtraTarget>>(&content).map_err(|error| {
            AppError::Other(format!("Failed to parse '{}': {error}", path.display()))
        })?;
    Ok(configured
        .into_iter()
        .map(|(name, target)| {
            let config = target.into_config(name.clone());
            (name, config)
        })
        .collect())
}

pub(crate) fn write_external_configs(
    configs: &BTreeMap<String, ExternalTargetConfig>,
) -> Result<()> {
    let mut serialized = BTreeMap::new();
    for (name, config) in configs {
        serialized.insert(
            name.clone(),
            ExtraTarget {
                id: config.id.clone(),
                resources: config
                    .resources
                    .iter()
                    .map(|resource| ExtraResource {
                        key: resource.key.clone(),
                        filename: resource.filename.clone(),
                        active_path: resource.active_path.clone(),
                        template: resource.template.clone(),
                        required: resource.required,
                    })
                    .collect(),
            },
        );
    }
    let content = toml::to_string(&serialized).map_err(|error| {
        AppError::Other(format!("Failed to serialize {EXTRA_TARGET_FILE}: {error}"))
    })?;
    let repository = config::repository_dir()?;
    let path = repository.join(EXTRA_TARGET_FILE);
    fs::create_dir_all(repository)?;
    fs::write(path, content)?;
    Ok(())
}

pub(crate) fn external_config_for(target: &TargetSpec) -> Result<Option<ExternalTargetConfig>> {
    let target_id = target.id;
    Ok(read_external_configs()?
        .into_values()
        .find(|config| config.resolved_id() == target_id))
}

pub(crate) fn find_external_config<'a>(
    configs: &'a BTreeMap<String, ExternalTargetConfig>,
    target_id: &str,
) -> Option<&'a ExternalTargetConfig> {
    configs
        .values()
        .find(|config| config.resolved_id() == target_id)
}

pub(crate) fn spec_from_external_config(config: &ExternalTargetConfig) -> Result<TargetSpec> {
    let target_id = config.resolved_id();
    if super::BUILTIN_TARGETS
        .iter()
        .any(|target| target.id == target_id)
    {
        return Err(AppError::TargetConflict(format!(
            "target id '{}' conflicts with an existing target",
            target_id
        )));
    }
    let mut keys = HashSet::new();
    for resource in &config.resources {
        if !keys.insert(resource.resolved_key()) {
            return Err(AppError::TargetConflict(format!(
                "target '{}' declares duplicate resource '{}'",
                target_id,
                resource.resolved_key()
            )));
        }
    }
    Ok(spec_from_external_config_unchecked(config))
}

fn spec_from_external_config_unchecked(config: &ExternalTargetConfig) -> TargetSpec {
    let id = leak_string(config.resolved_id().to_string());
    let resources = config
        .resources
        .iter()
        .map(ExternalResourceConfig::to_spec)
        .collect::<Vec<_>>();
    TargetSpec {
        id,
        resources: Box::leak(resources.into_boxed_slice()),
    }
}

pub(crate) fn merge_external_configs<F>(
    mut local: BTreeMap<String, ExternalTargetConfig>,
    packaged: &[ExternalTargetConfig],
    mut use_packaged: F,
) -> Result<BTreeMap<String, ExternalTargetConfig>>
where
    F: FnMut(&str) -> Result<bool>,
{
    for incoming in packaged {
        let incoming_id = incoming.resolved_id();
        let key = if let Some(existing) = local.get(&incoming.name) {
            if existing.resolved_id() != incoming_id {
                let prompt = format!(
                    "External target '{}' has conflicting ids ('{}' vs '{}'); use packaged definition?",
                    incoming.name,
                    existing.resolved_id(),
                    incoming_id
                );
                if use_packaged(&prompt)? {
                    local
                        .entry(incoming.name.clone())
                        .and_modify(|config| config.id = incoming.id.clone());
                } else {
                    let mut copied = incoming.clone();
                    if copied.id.is_none() {
                        copied.id = Some(copied.resolved_id().to_string());
                    }
                    let base = copied.name.clone();
                    let mut suffix = 2;
                    while local.contains_key(&copied.name) {
                        copied.name = format!("{base}-{suffix}");
                        suffix += 1;
                    }
                    local.insert(copied.name.clone(), copied);
                    continue;
                }
            }
            incoming.name.clone()
        } else if let Some((name, _)) = local
            .iter()
            .find(|(_, config)| config.resolved_id() == incoming_id)
        {
            name.clone()
        } else {
            local.insert(incoming.name.clone(), incoming.clone());
            continue;
        };

        let current = local.entry(key).or_insert_with(|| incoming.clone());
        for packaged_resource in &incoming.resources {
            let resource_key = packaged_resource.resolved_key();
            let Some(index) = current
                .resources
                .iter()
                .position(|resource| resource.resolved_key() == resource_key)
            else {
                current.resources.push(packaged_resource.clone());
                continue;
            };
            if current.resources[index].same_definition(packaged_resource) {
                continue;
            }
            let prompt = format!(
                "External target '{}' resource '{}' conflicts; use packaged definition?",
                incoming_id, resource_key
            );
            if use_packaged(&prompt)? {
                current.resources[index] = packaged_resource.clone();
            }
        }
    }
    Ok(local)
}

fn leak_string(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn leak_bytes(value: Vec<u8>) -> &'static [u8] {
    Box::leak(value.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ExternalResourceConfig, ExternalTargetConfig, ExtraTarget, merge_external_configs,
        spec_from_external_config,
    };

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

        let target = spec_from_external_config(&target.into_config("custom".to_string())).unwrap();
        assert_eq!(target.id, "custom-id");
        assert_eq!(target.resources[0].key, "settings.conf");
        assert!(target.resources[0].required);
        assert!(target.resources[0].template.is_empty());
        assert_eq!(target.resources[1].key, "auth");
        assert!(!target.resources[1].required);
        assert_eq!(target.resources[1].template, b"hello");
    }

    #[test]
    fn external_configs_merge_resources_and_resolve_conflicts() {
        let local = ExternalTargetConfig {
            name: "demo".to_string(),
            id: None,
            resources: vec![ExternalResourceConfig {
                key: Some("settings".to_string()),
                filename: "old.conf".to_string(),
                active_path: ".config/demo/old.conf".to_string(),
                template: None,
                required: None,
            }],
        };
        let packaged = ExternalTargetConfig {
            name: "demo".to_string(),
            id: None,
            resources: vec![
                ExternalResourceConfig {
                    key: Some("settings".to_string()),
                    filename: "new.conf".to_string(),
                    active_path: ".config/demo/new.conf".to_string(),
                    template: None,
                    required: None,
                },
                ExternalResourceConfig {
                    key: Some("auth".to_string()),
                    filename: "auth.json".to_string(),
                    active_path: ".config/demo/auth.json".to_string(),
                    template: None,
                    required: Some(false),
                },
            ],
        };
        let mut configs = BTreeMap::new();
        configs.insert(local.name.clone(), local);
        let merged = merge_external_configs(configs, &[packaged], |_| Ok(false)).unwrap();
        let merged = &merged["demo"];
        assert_eq!(merged.resources.len(), 2);
        assert_eq!(merged.resources[0].filename, "old.conf");
        assert_eq!(merged.resources[1].resolved_key(), "auth");
    }

    #[test]
    fn target_id_conflict_keeps_both_definitions_when_local_wins() {
        let local = ExternalTargetConfig {
            name: "demo".to_string(),
            id: Some("local-id".to_string()),
            resources: vec![],
        };
        let packaged = ExternalTargetConfig {
            name: "demo".to_string(),
            id: None,
            resources: vec![],
        };
        let mut configs = BTreeMap::new();
        configs.insert(local.name.clone(), local);
        let merged = merge_external_configs(configs, &[packaged], |_| Ok(false)).unwrap();
        assert!(
            merged
                .values()
                .any(|config| config.resolved_id() == "local-id")
        );
        assert!(merged.values().any(|config| config.resolved_id() == "demo"));
    }
}
