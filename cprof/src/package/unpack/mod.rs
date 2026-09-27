//! Prepare and commit package imports with caller-provided conflict decisions.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::error::{IoContext, PackageError, Result};
use crate::filesystem::transaction::PathTransaction;
use crate::targets::TargetRepository;
use crate::targets::external::{self, ConfigConflict, ExternalTargetConfig};

use super::{TargetPackage, decode_with_repository};

mod config;
mod fingerprint;
mod report;
mod target;

use config::{merge_target_configs, resolve_target, target_config_changes};
use fingerprint::Fingerprint;
pub(crate) use report::{
    ActiveChange, ChangeKind, ChangeSubject, UnpackChange, UnpackPreview, UnpackReport,
    UnpackedTarget,
};
use target::{TargetPlan, remap_profiles};

/// The caller supplies conflict policy and any progress presentation.
/// All callbacks run during preparation, before any changes are staged.
pub(crate) trait UnpackInteraction {
    fn use_packaged_config(&mut self, conflict: &ConfigConflict) -> Result<bool>;
    fn begin_target(&mut self, target: &str);
    fn overwrite_profile(&mut self, target: &str, name: &str) -> Result<bool>;
    fn remove_profile(&mut self, target: &str, name: &str) -> Result<bool>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnpackMode {
    Merge,
    Mirror,
}

impl From<bool> for UnpackMode {
    fn from(mirror: bool) -> Self {
        if mirror {
            UnpackMode::Mirror
        } else {
            UnpackMode::Merge
        }
    }
}

/// Prepare every packaged target, or only `target_id` when supplied.
pub(crate) fn prepare(
    targets: &TargetRepository,
    path: &Path,
    target_id: Option<&str>,
    mode: UnpackMode,
    interaction: &mut impl UnpackInteraction,
) -> Result<UnpackPlan> {
    if !path.exists() {
        return Err(PackageError::NotFound(path.to_path_buf()).into());
    }
    let bytes = fs::read(path).with_path(path)?;
    let packages = selected_packages(targets, &bytes, target_id)?;
    let fingerprint = Fingerprint::new(&bytes, targets, target_id, mode)?;
    UnpackPlan::prepare(targets, packages, target_id, mode, interaction, fingerprint)
}

/// Prepare an unpack without staging or committing any filesystem changes.
pub(crate) fn preview(
    targets: &TargetRepository,
    path: &Path,
    target_id: Option<&str>,
    mode: UnpackMode,
    interaction: &mut impl UnpackInteraction,
) -> Result<UnpackPreview> {
    Ok(prepare(targets, path, target_id, mode, interaction)?.preview_report())
}

// Keep validated profiles and the corresponding configuration change together
// so they can only be committed as a single transaction.
pub(crate) struct UnpackPlan {
    targets: Vec<TargetPlan>,
    target_configs: Option<BTreeMap<String, ExternalTargetConfig>>,
    target_config_changes: Vec<UnpackChange>,
    fingerprint: [u8; 32],
}

impl UnpackPlan {
    fn prepare(
        targets: &TargetRepository,
        packages: Vec<TargetPackage>,
        target_id: Option<&str>,
        mode: UnpackMode,
        interaction: &mut impl UnpackInteraction,
        mut fingerprint: Fingerprint,
    ) -> Result<Self> {
        let target_configs =
            merge_target_configs(targets, &packages, target_id, mode, interaction)?;
        let target_config_changes =
            target_config_changes(targets.external_configs(), &target_configs);
        let mut plans = Vec::with_capacity(packages.len());
        let packaged_targets = packages
            .iter()
            .map(|package| package.target.clone())
            .collect::<BTreeSet<_>>();
        for package in packages {
            let target = resolve_target(targets, &package, &target_configs)?;
            let active_profile = package.active_profile.clone();
            let active_profile_known = package.active_profile_known;
            let profiles = remap_profiles(&target, package.profiles)?;
            fingerprint.target(&target, &profiles, mode)?;
            plans.push(TargetPlan::prepare(
                target,
                profiles,
                active_profile,
                active_profile_known,
                mode,
                interaction,
            )?);
        }
        if mode == UnpackMode::Mirror && target_id.is_none() {
            for target in targets.all() {
                if packaged_targets.contains(&target.id) {
                    continue;
                }
                fingerprint.target(target, &[], mode)?;
                plans.push(TargetPlan::prepare(
                    target.clone(),
                    Vec::new(),
                    None,
                    true,
                    mode,
                    interaction,
                )?);
            }
        }
        Ok(Self {
            targets: plans,
            target_configs: (!target_config_changes.is_empty()).then_some(target_configs),
            target_config_changes,
            fingerprint: fingerprint.finish(),
        })
    }

    pub(crate) fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    pub(crate) fn commit(&self) -> Result<()> {
        PathTransaction::prepare(|transaction| {
            for plan in &self.targets {
                plan.stage(transaction)?;
            }
            if let Some(configs) = &self.target_configs {
                external::stage_external_configs(transaction, configs)?;
            }
            Ok(())
        })?
        .commit()
    }

    pub(crate) fn into_report(self) -> UnpackReport {
        UnpackReport {
            targets: self
                .targets
                .into_iter()
                .map(TargetPlan::into_report)
                .collect(),
        }
    }
}

fn selected_packages(
    targets: &TargetRepository,
    bytes: &[u8],
    target_id: Option<&str>,
) -> Result<Vec<TargetPackage>> {
    let packages = decode_with_repository(targets, bytes)?;
    match target_id {
        Some(target_id) => Ok(vec![
            packages
                .into_iter()
                .find(|package| package.target == target_id)
                .ok_or_else(|| {
                    PackageError::Invalid(format!("Package does not contain target '{target_id}'"))
                })?,
        ]),
        None => Ok(packages),
    }
}
