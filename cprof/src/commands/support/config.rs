use std::collections::BTreeMap;

use crate::error::Result;
use crate::targets::TargetRepository;
use crate::targets::external::ExternalTargetConfig;
use crate::ui::style;

pub(crate) fn load_targets() -> Result<TargetRepository> {
    let targets = TargetRepository::load()?;
    warn_redundant_paths(targets.external_configs());
    Ok(targets)
}

/// Present advice only after all definitions have passed environment validation.
pub(crate) fn warn_redundant_paths(configs: &BTreeMap<String, ExternalTargetConfig>) {
    for config in configs.values() {
        for resource in &config.resources {
            if resource.active_path.is_some()
                && let Some(path) = &resource.absolute_active_path
            {
                style::warning(format!(
                    "Target '{}' resource '{}' sets active_path and absolute_active_path to the same path ('{path}')",
                    config.resolved_id(),
                    resource.resolved_key()
                ));
            }
        }
    }
}
