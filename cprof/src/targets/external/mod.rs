use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str;

use serde::{Deserialize, Serialize};

use crate::cli;
use crate::config::{self, home, paths};
use crate::error::{ConfigError, IoContext, Result, TargetError};
use crate::filesystem::transaction::PathTransaction;
use crate::format::Format;
use crate::ui::style;

use super::{BUILTIN_TARGETS, ResourceSpec, TargetSpec};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ExternalTargetConfig {
    pub(crate) name: String,
    pub(crate) id: Option<String>,
    pub(crate) resources: Vec<ExternalResourceConfig>,
}

/// Stable conflict identity, independent of presentation text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum ConfigConflict {
    TargetId {
        name: String,
        local: String,
        packaged: String,
    },
    Resource {
        target: String,
        resource: String,
    },
}

impl std::fmt::Display for ConfigConflict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TargetId {
                name,
                local,
                packaged,
            } => write!(
                formatter,
                "External target '{name}' has conflicting ids ('{local}' vs '{packaged}'); use packaged definition?"
            ),
            Self::Resource { target, resource } => write!(
                formatter,
                "External target '{target}' resource '{resource}' conflicts; use packaged definition?"
            ),
        }
    }
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

    fn validate(&self, command_names: &[String], home: &Path) -> Result<()> {
        let target_id = self.resolved_id();
        // Validate both the table name and resolved id.  The table name is
        // retained in package definitions and must not be able to smuggle a
        // path component even when an explicit id is supplied.
        paths::validate_target_id(&self.name)?;
        paths::validate_target_id(target_id)?;
        if BUILTIN_TARGETS.iter().any(|target| target.id == target_id) {
            return Err(TargetError::Conflict(format!(
                "target id '{}' conflicts with an existing target",
                target_id
            ))
            .into());
        }
        if command_names.iter().any(|name| name == target_id) {
            return Err(TargetError::Conflict(format!(
                "target id '{}' conflicts with the command of the same name",
                target_id
            ))
            .into());
        }
        let mut keys = HashSet::new();
        let mut filenames = HashSet::new();
        let mut active_paths = HashMap::new();
        for resource in &self.resources {
            if resource.resolved_key().is_empty()
                || resource.resolved_key().chars().any(char::is_control)
            {
                return Err(TargetError::Conflict(format!(
                    "target '{}' declares an invalid empty or control-character resource key",
                    target_id
                ))
                .into());
            }
            if !keys.insert(resource.resolved_key()) {
                return Err(TargetError::Conflict(format!(
                    "target '{}' declares duplicate resource '{}'",
                    target_id,
                    resource.resolved_key()
                ))
                .into());
            }

            paths::validate_filename(&resource.filename)?;
            if !filenames.insert(resource.filename.as_str()) {
                return Err(TargetError::Conflict(format!(
                    "target '{}' declares duplicate filename '{}'",
                    target_id, resource.filename
                ))
                .into());
            }

            let active_path = resource.effective_active_path(target_id, home)?;
            if let Some(previous) = active_paths.insert(active_path, resource.resolved_key()) {
                return Err(TargetError::Conflict(format!(
                    "target '{}' declares resources '{}' and '{}' with the same active path",
                    target_id,
                    previous,
                    resource.resolved_key()
                ))
                .into());
            }
        }
        Ok(())
    }

    /// Convert without revalidating; the definition must already have passed validation.
    pub(super) fn to_spec_unchecked(&self) -> TargetSpec {
        let resources = self
            .resources
            .iter()
            .map(ExternalResourceConfig::to_spec)
            .collect::<Vec<_>>();
        TargetSpec {
            id: self.resolved_id().to_string(),
            resources,
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

    fn effective_active_path(&self, target_id: &str, home: &Path) -> Result<PathBuf> {
        let path = paths::resolve_active_path(
            home,
            self.active_path.as_deref(),
            self.absolute_active_path.as_deref(),
        )
        .map_err(|error| {
            TargetError::Conflict(format!(
                "target '{target_id}' resource '{}': {error}",
                self.resolved_key()
            ))
        })?;

        if self.active_path.is_some() && self.absolute_active_path.is_some() {
            style::warning(format!(
                "Target '{target_id}' resource '{}' sets active_path and absolute_active_path to the same path ('{}')",
                self.resolved_key(),
                path.display()
            ));
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
        ResourceSpec {
            key: self.key.clone().unwrap_or_else(|| self.filename.clone()),
            filename: self.filename.clone(),
            active_path: self.active_path.clone(),
            absolute_active_path: self.absolute_active_path.clone(),
            required: self.required.unwrap_or(true),
            template: self.template.clone().unwrap_or_default().into_bytes(),
            format: Format::Any,
        }
    }
}

fn validate_external_configs(configs: &BTreeMap<String, ExternalTargetConfig>) -> Result<()> {
    let command_names = cli::command_names();
    let home = home::dir()?;
    let mut ids = HashSet::new();
    for (name, config) in configs {
        // The TOML table key is the external definition's name and is also
        // used as the merge key.  Keep it in the same safe-component domain
        // as the resolved target id, and reject hand-built maps whose key and
        // definition disagree instead of serializing an ambiguous config.
        paths::validate_target_id(name)?;
        if config.name != *name {
            return Err(TargetError::Conflict(format!(
                "target definition name '{}' does not match table name '{}'",
                config.name, name
            ))
            .into());
        }
        config.validate(command_names, &home)?;
        if !ids.insert(config.resolved_id()) {
            return Err(TargetError::Conflict(format!(
                "target id '{}' declared by '{name}' conflicts with an existing target",
                config.resolved_id()
            ))
            .into());
        }
    }
    Ok(())
}

/// Validate one embedded or local definition with the same rules used for the
/// complete TOML map. Package manifests use this boundary directly so a
/// malformed embedded definition cannot depend on the caller's merge mode.
pub(crate) fn validate_external_config(config: &ExternalTargetConfig) -> Result<()> {
    config.validate(cli::command_names(), &home::dir()?)
}

// Keep validation at the read/write boundary: unpack also accesses configs without all().
pub(crate) fn read_external_configs() -> Result<BTreeMap<String, ExternalTargetConfig>> {
    let path = config::extra_target_file()?;
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    parse_external_configs(&fs::read(&path).with_path(&path)?, &path)
}

pub(crate) fn validate_external_configs_content(content: &[u8]) -> Result<()> {
    parse_external_configs(content, &config::extra_target_file()?).map(|_| ())
}

fn parse_external_configs(
    content: &[u8],
    path: &Path,
) -> Result<BTreeMap<String, ExternalTargetConfig>> {
    let content = str::from_utf8(content).map_err(|source| ConfigError::Encoding {
        path: path.to_path_buf(),
        source,
    })?;
    let configured =
        toml::from_str::<BTreeMap<String, ExtraTarget>>(content).map_err(|source| {
            ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            }
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

pub(crate) fn stage_external_configs(
    transaction: &mut PathTransaction,
    configs: &BTreeMap<String, ExternalTargetConfig>,
) -> Result<()> {
    let content = serialize_external_configs(configs)?;
    let path = config::extra_target_file()?;
    transaction
        .stage_file(&path, "stage", &content, path.exists())
        .map(|_| ())
}

fn serialize_external_configs(configs: &BTreeMap<String, ExternalTargetConfig>) -> Result<Vec<u8>> {
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
    let content = toml::to_string(&serialized).map_err(ConfigError::Serialize)?;
    Ok(content.into_bytes())
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
    F: FnMut(&ConfigConflict) -> Result<bool>,
{
    // Validate before any conflict prompt or `force` decision.  This keeps
    // merge callers from creating a validation bypass by supplying configs
    // directly instead of going through the TOML/manifest readers.
    validate_external_configs(&local)?;
    let command_names = cli::command_names();
    let home = home::dir()?;
    for config in packaged {
        config.validate(command_names, &home)?;
    }

    for incoming in packaged {
        let incoming_id = incoming.resolved_id();
        let key = if let Some(existing) = local.get(&incoming.name) {
            if existing.resolved_id() != incoming_id {
                let conflict = ConfigConflict::TargetId {
                    name: incoming.name.clone(),
                    local: existing.resolved_id().into(),
                    packaged: incoming_id.into(),
                };
                if use_packaged(&conflict)? {
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
            let conflict = ConfigConflict::Resource {
                target: incoming_id.into(),
                resource: resource_key.into(),
            };
            if use_packaged(&conflict)? {
                current.resources[index] = packaged_resource.clone();
            }
        }
    }
    validate_external_configs(&local)?;
    Ok(local)
}

/// Build the exact external target set for a mirror unpack. Root mirrors use
/// the package set as-is; target mirrors replace only the selected target and
/// preserve unrelated local definitions.
pub(crate) fn replace_external_configs(
    mut local: BTreeMap<String, ExternalTargetConfig>,
    packaged: &[ExternalTargetConfig],
    selected_target: Option<&str>,
) -> Result<BTreeMap<String, ExternalTargetConfig>> {
    let command_names = cli::command_names();
    let home = home::dir()?;
    for config in packaged {
        config.validate(command_names, &home)?;
    }

    if selected_target.is_none() {
        let mut replaced = BTreeMap::new();
        for config in packaged {
            if replaced
                .insert(config.name.clone(), config.clone())
                .is_some()
            {
                return Err(TargetError::Conflict(format!(
                    "duplicate external target definition '{}'",
                    config.name
                ))
                .into());
            }
        }
        validate_external_configs(&replaced)?;
        return Ok(replaced);
    }

    let target_id = selected_target.expect("selected target is present");
    for incoming in packaged {
        if incoming.resolved_id() != target_id {
            continue;
        }
        local.retain(|name, config| name != &incoming.name && config.resolved_id() != target_id);
        local.insert(incoming.name.clone(), incoming.clone());
    }
    validate_external_configs(&local)?;
    Ok(local)
}

#[cfg(test)]
mod tests;
