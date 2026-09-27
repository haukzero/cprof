use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::error::{PackageError, Result};
use crate::package::TargetPackage;
use crate::targets::external::{self, ExternalTargetConfig};
use crate::targets::{TargetRepository, TargetSpec};

use super::{ChangeKind, ChangeSubject, UnpackChange, UnpackInteraction, UnpackMode};

pub(super) fn target_config_changes(
    current: &BTreeMap<String, ExternalTargetConfig>,
    merged: &BTreeMap<String, ExternalTargetConfig>,
) -> Vec<UnpackChange> {
    let mut names = BTreeSet::new();
    names.extend(current.keys());
    names.extend(merged.keys());

    names
        .into_iter()
        .filter_map(|name| {
            let (kind, target) = match (current.get(name), merged.get(name)) {
                (None, Some(config)) => (ChangeKind::Added, config.resolved_id()),
                (Some(config), None) => (ChangeKind::Deleted, config.resolved_id()),
                (Some(current), Some(merged)) if current != merged => {
                    (ChangeKind::Modified, merged.resolved_id())
                }
                _ => return None,
            };
            Some(UnpackChange {
                kind,
                subject: ChangeSubject::TargetDefinition {
                    target: target.to_string(),
                },
            })
        })
        .collect()
}

pub(super) fn merge_target_configs(
    targets: &TargetRepository,
    packages: &[TargetPackage],
    target_id: Option<&str>,
    mode: UnpackMode,
    interaction: &mut impl UnpackInteraction,
) -> Result<BTreeMap<String, ExternalTargetConfig>> {
    let packaged = packages
        .iter()
        .filter_map(|package| package.target_config.clone())
        .collect::<Vec<_>>();
    match mode {
        UnpackMode::Merge => external::merge_external_configs(
            targets.external_configs().clone(),
            &packaged,
            |conflict| interaction.use_packaged_config(conflict),
        ),
        UnpackMode::Mirror => external::replace_external_configs(
            targets.external_configs().clone(),
            &packaged,
            target_id,
        ),
    }
}

pub(super) fn resolve_target(
    targets: &TargetRepository,
    package: &TargetPackage,
    configs: &BTreeMap<String, ExternalTargetConfig>,
) -> Result<Arc<TargetSpec>> {
    if package.target_config.is_none() {
        return targets.get(&package.target);
    }
    let config = external::find_external_config(configs, &package.target).ok_or_else(|| {
        PackageError::Invalid(format!(
            "Merged configuration does not contain target '{}'",
            package.target
        ))
    })?;
    Ok(Arc::new(external::spec_from_external_config(config)?))
}
