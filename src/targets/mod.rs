mod claude;
mod codex;
pub(crate) mod external;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use crate::config::paths;
use crate::error::{Result, TargetError};
use crate::format::Format;

use external::ExternalTargetConfig;

#[derive(Debug, Clone)]
pub struct ResourceSpec {
    pub key: String,
    pub filename: String,
    /// Path relative to the user's home directory, when configured.
    ///
    /// External targets may instead use `absolute_active_path`; built-in
    /// targets use this field exclusively.
    pub active_path: Option<String>,
    /// Absolute path used for resources that do not live below `$HOME`.
    ///
    /// This is intentionally optional so the same representation can be used
    /// for both built-in and external target definitions.
    pub absolute_active_path: Option<String>,
    pub required: bool,
    pub template: Vec<u8>,
    pub format: Format,
}

impl ResourceSpec {
    pub fn validate(&self, bytes: &[u8]) -> Result<()> {
        self.format.check(bytes)
    }
}

#[derive(Debug, Clone)]
pub struct TargetSpec {
    pub id: String,
    pub resources: Vec<ResourceSpec>,
}

impl TargetSpec {
    pub fn resource(&self, key: &str) -> Result<&ResourceSpec> {
        self.resources
            .iter()
            .find(|resource| resource.key == key)
            .ok_or_else(|| {
                TargetError::UnknownResource {
                    target: self.id.to_string(),
                    resource: key.to_string(),
                }
                .into()
            })
    }

    /// Resolve the active path, checking its syntax and existing parent directories.
    pub fn active_path(&self, home: &Path, resource: &ResourceSpec) -> Result<PathBuf> {
        paths::validate_target_id(&self.id)?;
        paths::resolve_active_path(
            home,
            resource.active_path.as_deref(),
            resource.absolute_active_path.as_deref(),
        )
    }
}

static BUILTIN_TARGETS: LazyLock<Vec<Arc<TargetSpec>>> =
    LazyLock::new(|| vec![Arc::new(claude::spec()), Arc::new(codex::spec())]);

/// Target definitions owned for the lifetime of one application operation.
///
/// Loading a repository takes a fresh snapshot of the external configuration.
/// Callers that perform several target operations should share one repository.
#[derive(Debug)]
pub struct TargetRepository {
    targets: Vec<Arc<TargetSpec>>,
    external_configs: BTreeMap<String, ExternalTargetConfig>,
}

impl TargetRepository {
    pub fn load() -> Result<Self> {
        let external_configs = external::read_external_configs()?;
        let mut targets = BUILTIN_TARGETS.iter().cloned().collect::<Vec<_>>();
        targets.extend(
            external_configs
                .values()
                .map(|config| Arc::new(config.to_spec_unchecked())),
        );
        Ok(Self {
            targets,
            external_configs,
        })
    }

    pub fn get(&self, id: &str) -> Result<Arc<TargetSpec>> {
        self.targets
            .iter()
            .find(|target| target.id == id)
            .cloned()
            .ok_or_else(|| TargetError::Unknown(id.to_string()).into())
    }

    pub fn all(&self) -> &[Arc<TargetSpec>] {
        &self.targets
    }

    pub(crate) fn external_configs(&self) -> &BTreeMap<String, ExternalTargetConfig> {
        &self.external_configs
    }

    pub(crate) fn external_config_for(&self, target: &TargetSpec) -> Option<&ExternalTargetConfig> {
        self.external_configs
            .values()
            .find(|config| config.resolved_id() == target.id)
    }
}

pub(crate) fn is_builtin(target: &TargetSpec) -> bool {
    BUILTIN_TARGETS
        .iter()
        .any(|builtin| builtin.id == target.id)
}

#[cfg(test)]
mod tests {
    use super::{claude, codex, is_builtin};

    #[test]
    fn builtin_targets_are_identified() {
        assert!(is_builtin(&claude::spec()));
        assert!(is_builtin(&codex::spec()));
    }
}
