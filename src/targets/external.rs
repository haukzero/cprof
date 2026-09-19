use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::{AppError, Result};
use crate::paths;
use crate::style;

use super::{EXTRA_TARGET_FILE, ResourceSpec, TargetSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalTargetConfig {
    pub(crate) name: String,
    pub(crate) id: Option<String>,
    pub(crate) resources: Vec<ExternalResourceConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct ExternalResourceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) key: Option<String>,
    pub(crate) filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) active_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) absolute_active_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) required: Option<bool>,
}

impl ExternalTargetConfig {
    pub(crate) fn resolved_id(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.name)
    }

    fn validate(&self, command_names: &[String]) -> Result<()> {
        let target_id = self.resolved_id();
        // Validate both the table name and resolved id.  The table name is
        // retained in package definitions and must not be able to smuggle a
        // path component even when an explicit id is supplied.
        paths::validate_target_id(&self.name)?;
        paths::validate_target_id(target_id)?;
        if super::BUILTIN_TARGETS
            .iter()
            .any(|target| target.id == target_id)
        {
            return Err(AppError::TargetConflict(format!(
                "target id '{}' conflicts with an existing target",
                target_id
            )));
        }
        if command_names.iter().any(|name| name == target_id) {
            return Err(AppError::TargetConflict(format!(
                "target id '{}' conflicts with the command of the same name",
                target_id
            )));
        }
        let mut keys = HashSet::new();
        let mut filenames = HashSet::new();
        let mut active_paths = HashMap::new();
        for resource in &self.resources {
            if !keys.insert(resource.resolved_key()) {
                return Err(AppError::TargetConflict(format!(
                    "target '{}' declares duplicate resource '{}'",
                    target_id,
                    resource.resolved_key()
                )));
            }

            paths::validate_filename(&resource.filename)?;
            if !filenames.insert(resource.filename.as_str()) {
                return Err(AppError::TargetConflict(format!(
                    "target '{}' declares duplicate filename '{}'",
                    target_id, resource.filename
                )));
            }

            let active_path = resource.effective_active_path(target_id)?;
            if let Some(previous) = active_paths.insert(active_path, resource.resolved_key()) {
                return Err(AppError::TargetConflict(format!(
                    "target '{}' declares resources '{}' and '{}' with the same active path",
                    target_id,
                    previous,
                    resource.resolved_key()
                )));
            }
        }
        Ok(())
    }

    /// Convert without revalidating; the definition must already have passed validation.
    pub(super) fn to_spec_unchecked(&self) -> TargetSpec {
        let id = leak_string(self.resolved_id().to_string());
        let resources = self
            .resources
            .iter()
            .map(ExternalResourceConfig::to_spec)
            .collect::<Vec<_>>();
        TargetSpec {
            id,
            resources: Box::leak(resources.into_boxed_slice()),
        }
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
            && self.absolute_active_path == other.absolute_active_path
            && self.template.as_deref().unwrap_or("") == other.template.as_deref().unwrap_or("")
            && self.required.unwrap_or(true) == other.required.unwrap_or(true)
    }

    fn effective_active_path(&self, target_id: &str) -> Result<PathBuf> {
        let home = config::home_dir()?;
        let path = paths::resolve_active_path(
            &home,
            self.active_path.as_deref(),
            self.absolute_active_path.as_deref(),
        )
        .map_err(|error| {
            AppError::TargetConflict(format!(
                "target '{target_id}' resource '{}': {error}",
                self.resolved_key()
            ))
        })?;

        if self.active_path.is_some() && self.absolute_active_path.is_some() {
            eprintln!(
                "{}",
                style::warning(&format!(
                    "Target '{target_id}' resource '{}' sets active_path and absolute_active_path to the same path ('{}')",
                    self.resolved_key(),
                    path.display()
                ))
            );
        }
        paths::path_identity(&path)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ExtraTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    resources: Vec<ExternalResourceConfig>,
}

impl ExtraTarget {
    fn into_config(self, name: String) -> ExternalTargetConfig {
        ExternalTargetConfig {
            name,
            id: self.id,
            resources: self.resources,
        }
    }
}

impl ExternalResourceConfig {
    fn to_spec(&self) -> ResourceSpec {
        let filename = leak_string(self.filename.clone());
        let key = leak_string(self.key.clone().unwrap_or_else(|| self.filename.clone()));
        let template = leak_bytes(self.template.clone().unwrap_or_default().into_bytes());
        ResourceSpec {
            key,
            filename,
            active_path: self.active_path.clone().map(leak_string),
            absolute_active_path: self.absolute_active_path.clone().map(leak_string),
            required: self.required.unwrap_or(true),
            template,
            validate: super::external_validate,
        }
    }
}

fn validate_external_configs(configs: &BTreeMap<String, ExternalTargetConfig>) -> Result<()> {
    let command_names = crate::cli::command_names();
    let mut ids = HashSet::new();
    for (name, config) in configs {
        // The TOML table key is the external definition's name and is also
        // used as the merge key.  Keep it in the same safe-component domain
        // as the resolved target id, and reject hand-built maps whose key and
        // definition disagree instead of serializing an ambiguous config.
        paths::validate_target_id(name)?;
        if config.name != *name {
            return Err(AppError::TargetConflict(format!(
                "target definition name '{}' does not match table name '{}'",
                config.name, name
            )));
        }
        config.validate(command_names)?;
        if !ids.insert(config.resolved_id()) {
            return Err(AppError::TargetConflict(format!(
                "target id '{}' declared by '{name}' conflicts with an existing target",
                config.resolved_id()
            )));
        }
    }
    Ok(())
}

/// Validate one embedded or local definition with the same rules used for the
/// complete TOML map. Package manifests use this boundary directly so a
/// malformed embedded definition cannot depend on the caller's merge mode.
pub(crate) fn validate_external_config(config: &ExternalTargetConfig) -> Result<()> {
    config.validate(crate::cli::command_names())
}

// Keep validation at the read/write boundary: unpack also accesses configs without all().
pub(crate) fn read_external_configs() -> Result<BTreeMap<String, ExternalTargetConfig>> {
    let path = config::extra_target_file()?;
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let content = fs::read_to_string(&path)?;
    let configured =
        toml::from_str::<BTreeMap<String, ExtraTarget>>(&content).map_err(|error| {
            AppError::Other(format!("Failed to parse '{}': {error}", path.display()))
        })?;
    let configs = configured
        .into_iter()
        .map(|(name, target)| {
            let config = target.into_config(name.clone());
            (name, config)
        })
        .collect();
    validate_external_configs(&configs)?;
    Ok(configs)
}

pub(crate) fn write_external_configs(
    configs: &BTreeMap<String, ExternalTargetConfig>,
) -> Result<()> {
    validate_external_configs(configs)?;
    let mut serialized = BTreeMap::new();
    for (name, config) in configs {
        serialized.insert(
            name.clone(),
            ExtraTarget {
                id: config.id.clone(),
                resources: config.resources.clone(),
            },
        );
    }
    let content = toml::to_string(&serialized).map_err(|error| {
        AppError::Other(format!("Failed to serialize {EXTRA_TARGET_FILE}: {error}"))
    })?;
    let repository = config::repository_dir()?;
    let path = config::extra_target_file()?;
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
    validate_external_config(config)?;
    Ok(config.to_spec_unchecked())
}

pub(crate) fn merge_external_configs<F>(
    mut local: BTreeMap<String, ExternalTargetConfig>,
    packaged: &[ExternalTargetConfig],
    mut use_packaged: F,
) -> Result<BTreeMap<String, ExternalTargetConfig>>
where
    F: FnMut(&str) -> Result<bool>,
{
    // Validate before any conflict prompt or `force` decision.  This keeps
    // merge callers from creating a validation bypass by supplying configs
    // directly instead of going through the TOML/manifest readers.
    validate_external_configs(&local)?;
    let command_names = crate::cli::command_names();
    for config in packaged {
        config.validate(command_names)?;
    }

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
    validate_external_configs(&local)?;
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

    use crate::config;

    use super::{
        ExternalResourceConfig, ExternalTargetConfig, ExtraTarget, merge_external_configs,
        spec_from_external_config,
    };

    #[test]
    fn external_target_ids_cannot_shadow_root_commands() {
        for name in crate::cli::command_names() {
            let config = ExternalTargetConfig {
                name: "example".to_string(),
                id: Some(name.clone()),
                resources: vec![],
            };
            assert!(matches!(
                spec_from_external_config(&config),
                Err(crate::error::AppError::TargetConflict(_))
            ));

            // A table name may match a command when its explicit id does not.
            let config = ExternalTargetConfig {
                name: name.clone(),
                id: Some("custom-target".to_string()),
                resources: vec![],
            };
            assert_eq!(
                spec_from_external_config(&config).unwrap().id,
                "custom-target"
            );
        }
    }

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
                active_path: Some(".config/demo/old.conf".to_string()),
                absolute_active_path: None,
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
                    active_path: Some(".config/demo/new.conf".to_string()),
                    absolute_active_path: None,
                    template: None,
                    required: None,
                },
                ExternalResourceConfig {
                    key: Some("auth".to_string()),
                    filename: "auth.json".to_string(),
                    active_path: Some(".config/demo/auth.json".to_string()),
                    absolute_active_path: None,
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

    fn resource(
        key: &str,
        filename: &str,
        active_path: Option<&str>,
        absolute_active_path: Option<&str>,
    ) -> ExternalResourceConfig {
        ExternalResourceConfig {
            key: Some(key.to_string()),
            filename: filename.to_string(),
            active_path: active_path.map(str::to_string),
            absolute_active_path: absolute_active_path.map(str::to_string),
            template: None,
            required: None,
        }
    }

    fn target(resources: Vec<ExternalResourceConfig>) -> ExternalTargetConfig {
        ExternalTargetConfig {
            name: "demo".to_string(),
            id: None,
            resources,
        }
    }

    #[test]
    fn resource_paths_require_one_safe_active_path() {
        let invalid = [
            resource("r", "settings.json", None, None),
            resource("r", "settings/name.json", Some(".config/name"), None),
            resource("r", "settings.json", Some("../escape"), None),
            resource(
                "r",
                "settings.json",
                Some(".config/name"),
                Some("relative/name"),
            ),
            resource(
                "r",
                "settings.json",
                Some(".config/name"),
                Some("/tmp/other"),
            ),
        ];
        for resource in invalid {
            assert!(spec_from_external_config(&target(vec![resource])).is_err());
        }
    }

    #[test]
    fn absolute_active_path_is_supported_and_same_path_is_accepted() {
        let home = config::home_dir().unwrap();
        let absolute = home.join(".config/demo/settings.json");
        let config = target(vec![resource(
            "settings",
            "settings.json",
            Some(".config/demo/settings.json"),
            Some(absolute.to_str().unwrap()),
        )]);
        let spec = spec_from_external_config(&config).unwrap();
        assert_eq!(
            spec.resources[0].active_path,
            Some(".config/demo/settings.json")
        );
        assert_eq!(
            spec.resources[0].absolute_active_path,
            Some(absolute.to_str().unwrap())
        );
    }

    #[test]
    fn duplicate_filename_and_active_path_are_rejected() {
        let duplicate_filename = target(vec![
            resource("one", "same.json", Some(".config/demo/one.json"), None),
            resource("two", "same.json", Some(".config/demo/two.json"), None),
        ]);
        assert!(spec_from_external_config(&duplicate_filename).is_err());

        let duplicate_active_path = target(vec![
            resource("one", "one.json", Some(".config/demo/same.json"), None),
            resource("two", "two.json", Some(".config/demo/same.json"), None),
        ]);
        assert!(spec_from_external_config(&duplicate_active_path).is_err());
    }

    #[test]
    fn target_and_resource_components_are_rejected() {
        for id in ["", ".", "..", "a/b", "a\\b", "C:\\tmp"] {
            let config = ExternalTargetConfig {
                name: "demo".to_string(),
                id: Some(id.to_string()),
                resources: vec![],
            };
            assert!(spec_from_external_config(&config).is_err(), "{id:?}");
        }
        let invalid_filename = target(vec![resource(
            "r",
            "../settings.json",
            Some(".config/demo/settings.json"),
            None,
        )]);
        assert!(spec_from_external_config(&invalid_filename).is_err());
    }
}
