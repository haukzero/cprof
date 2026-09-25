//! Prepare and commit package imports, with caller-provided conflict decisions.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::error::{IoContext, PackageError, Result};
use crate::filesystem::transaction::PathTransaction;
use crate::profile::{self, storage};
use crate::targets::external::{self, ExternalTargetConfig};
use crate::targets::{TargetRepository, TargetSpec};

use super::{PackageProfile, TargetPackage, decode_with_repository};

/// The caller supplies conflict policy and any progress presentation.
/// All callbacks run during preparation, before any changes are staged.
pub(crate) trait UnpackInteraction {
    fn use_packaged_config(&mut self, conflict: &str) -> Result<bool>;
    fn begin_target(&mut self, target: &str);
    fn overwrite_profile(&mut self, name: &str) -> Result<bool>;
}

/// Returned only after all selected targets and their definitions are committed.
pub(crate) struct UnpackReport {
    pub(crate) targets: Vec<UnpackedTarget>,
}

pub(crate) struct UnpackedTarget {
    pub(crate) target: String,
    pub(crate) profiles: Vec<String>,
    pub(crate) skipped: usize,
}

impl UnpackReport {
    pub(crate) fn profile_count(&self) -> usize {
        self.targets
            .iter()
            .map(|target| target.profiles.len())
            .sum()
    }

    pub(crate) fn skipped_count(&self) -> usize {
        self.targets.iter().map(|target| target.skipped).sum()
    }
}

/// Restore every packaged target, or only `target_id` when supplied.
pub(crate) fn restore(
    targets: &TargetRepository,
    path: &Path,
    target_id: Option<&str>,
    interaction: &mut impl UnpackInteraction,
) -> Result<UnpackReport> {
    let packages = read_package(targets, path)?;
    let packages = match target_id {
        Some(target_id) => vec![
            packages
                .into_iter()
                .find(|package| package.target == target_id)
                .ok_or_else(|| {
                    PackageError::Invalid(format!("Package does not contain target '{target_id}'"))
                })?,
        ],
        None => packages,
    };
    UnpackPlan::prepare(targets, packages, interaction)?.commit()
}

// Keep validated profiles and the corresponding configuration change together
// so they can only be committed as a single transaction.
struct UnpackPlan {
    targets: Vec<TargetPlan>,
    target_configs: Option<BTreeMap<String, ExternalTargetConfig>>,
}

impl UnpackPlan {
    fn prepare(
        targets: &TargetRepository,
        packages: Vec<TargetPackage>,
        interaction: &mut impl UnpackInteraction,
    ) -> Result<Self> {
        let target_configs = merge_target_configs(targets, &packages, interaction)?;
        let mut plans = Vec::with_capacity(packages.len());
        for package in packages {
            let target = resolve_target(targets, &package, &target_configs)?;
            let profiles = remap_profiles(&target, package.profiles)?;
            plans.push(TargetPlan::prepare(target, profiles, interaction)?);
        }
        let configs_changed = &target_configs != targets.external_configs();
        Ok(Self {
            targets: plans,
            target_configs: configs_changed.then_some(target_configs),
        })
    }

    fn commit(self) -> Result<UnpackReport> {
        PathTransaction::prepare(|transaction| {
            for plan in &self.targets {
                plan.stage(transaction)?;
            }
            if let Some(configs) = &self.target_configs {
                external::stage_external_configs(transaction, configs)?;
            }
            Ok(())
        })?
        .commit()?;
        Ok(UnpackReport {
            targets: self
                .targets
                .into_iter()
                .map(TargetPlan::into_report)
                .collect(),
        })
    }
}

struct TargetPlan {
    target: Arc<TargetSpec>,
    profiles: Vec<PlannedProfile>,
    skipped: usize,
}

struct PlannedProfile {
    package: PackageProfile,
    overwrite: bool,
}

impl TargetPlan {
    fn prepare(
        target: Arc<TargetSpec>,
        profiles: Vec<PackageProfile>,
        interaction: &mut impl UnpackInteraction,
    ) -> Result<Self> {
        interaction.begin_target(&target.id);
        let mut planned = Vec::with_capacity(profiles.len());
        let mut skipped = 0;
        for package_profile in profiles {
            let exists = storage::exists(&target, &package_profile.name)?;
            if exists && !interaction.overwrite_profile(&package_profile.name)? {
                skipped += 1;
                continue;
            }
            storage::validate_replacement(
                &target,
                &package_profile.name,
                &package_profile.resources,
                exists,
            )?;
            planned.push(PlannedProfile {
                package: package_profile,
                overwrite: exists,
            });
        }
        Ok(Self {
            target,
            profiles: planned,
            skipped,
        })
    }

    fn stage(&self, transaction: &mut PathTransaction) -> Result<()> {
        for planned in &self.profiles {
            storage::stage_validated_replace(
                transaction,
                &self.target,
                &planned.package.name,
                &planned.package.resources,
                planned.overwrite,
            )?;
        }
        Ok(())
    }

    fn into_report(self) -> UnpackedTarget {
        UnpackedTarget {
            target: self.target.id.clone(),
            profiles: self
                .profiles
                .into_iter()
                .map(|profile| profile.package.name)
                .collect(),
            skipped: self.skipped,
        }
    }
}

fn read_package(targets: &TargetRepository, path: &Path) -> Result<Vec<TargetPackage>> {
    if !path.exists() {
        return Err(PackageError::NotFound(path.to_path_buf()).into());
    }
    decode_with_repository(targets, &fs::read(path).with_path(path)?)
}

fn merge_target_configs(
    targets: &TargetRepository,
    packages: &[TargetPackage],
    interaction: &mut impl UnpackInteraction,
) -> Result<BTreeMap<String, ExternalTargetConfig>> {
    let packaged = packages
        .iter()
        .filter_map(|package| package.target_config.clone())
        .collect::<Vec<_>>();
    external::merge_external_configs(targets.external_configs().clone(), &packaged, |conflict| {
        interaction.use_packaged_config(conflict)
    })
}

fn resolve_target(
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

fn remap_profiles(
    target: &TargetSpec,
    profiles: Vec<PackageProfile>,
) -> Result<Vec<PackageProfile>> {
    profiles
        .into_iter()
        .map(|profile| {
            let mut present = HashSet::new();
            let mut resources = profile
                .resources
                .into_iter()
                .map(|resource| {
                    let spec = target.resource(&resource.spec.key)?;
                    present.insert(spec.key.clone());
                    Ok(profile::ProfileResource {
                        spec: spec.clone(),
                        content: resource.content,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            for spec in &target.resources {
                if spec.required && !present.contains(&spec.key) {
                    resources.push(profile::ProfileResource {
                        spec: spec.clone(),
                        content: spec.template.clone(),
                    });
                }
            }
            Ok(PackageProfile::new(profile.name, resources))
        })
        .collect()
}
