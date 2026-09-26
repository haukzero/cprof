//! Prepare and commit package imports, with caller-provided conflict decisions.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::config;
use crate::error::{AppError, IoContext, PackageError, PathError, Result};
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

/// Describes the targets that were unpacked after a successful commit.
pub(crate) struct UnpackReport {
    pub(crate) targets: Vec<UnpackedTarget>,
}

pub(crate) struct UnpackPreview {
    pub(crate) changes: Vec<UnpackChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ChangeSubject {
    TargetDefinition { target: String },
    Profile { target: String, profile: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnpackChange {
    pub(crate) kind: ChangeKind,
    pub(crate) subject: ChangeSubject,
}

pub(crate) struct UnpackedTarget {
    pub(crate) target: String,
    pub(crate) profiles: Vec<String>,
    pub(crate) unchanged: usize,
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

    pub(crate) fn unchanged_count(&self) -> usize {
        self.targets.iter().map(|target| target.unchanged).sum()
    }
}

/// Restore every packaged target, or only `target_id` when supplied.
pub(crate) fn restore(
    targets: &TargetRepository,
    path: &Path,
    target_id: Option<&str>,
    interaction: &mut impl UnpackInteraction,
) -> Result<UnpackReport> {
    let packages = selected_packages(targets, path, target_id)?;
    UnpackPlan::prepare(targets, packages, interaction)?.commit()
}

/// Prepare an unpack without staging or committing any filesystem changes.
pub(crate) fn preview(
    targets: &TargetRepository,
    path: &Path,
    target_id: Option<&str>,
    interaction: &mut impl UnpackInteraction,
) -> Result<UnpackPreview> {
    let packages = selected_packages(targets, path, target_id)?;
    Ok(UnpackPlan::prepare(targets, packages, interaction)?.preview_report())
}

// Keep validated profiles and the corresponding configuration change together
// so they can only be committed as a single transaction.
struct UnpackPlan {
    targets: Vec<TargetPlan>,
    target_configs: Option<BTreeMap<String, ExternalTargetConfig>>,
    target_config_changes: Vec<UnpackChange>,
}

impl UnpackPlan {
    fn prepare(
        targets: &TargetRepository,
        packages: Vec<TargetPackage>,
        interaction: &mut impl UnpackInteraction,
    ) -> Result<Self> {
        let target_configs = merge_target_configs(targets, &packages, interaction)?;
        let target_config_changes =
            target_config_changes(targets.external_configs(), &target_configs);
        let mut plans = Vec::with_capacity(packages.len());
        for package in packages {
            let target = resolve_target(targets, &package, &target_configs)?;
            let profiles = remap_profiles(&target, package.profiles)?;
            plans.push(TargetPlan::prepare(target, profiles, interaction)?);
        }
        Ok(Self {
            targets: plans,
            target_configs: (!target_config_changes.is_empty()).then_some(target_configs),
            target_config_changes,
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
        Ok(self.into_report())
    }

    fn into_report(self) -> UnpackReport {
        UnpackReport {
            targets: self
                .targets
                .into_iter()
                .map(TargetPlan::into_report)
                .collect(),
        }
    }

    fn preview_report(&self) -> UnpackPreview {
        let mut changes = self.target_config_changes.clone();
        for plan in &self.targets {
            for profile in &plan.profiles {
                changes.push(UnpackChange {
                    kind: if profile.overwrite {
                        ChangeKind::Modified
                    } else {
                        ChangeKind::Added
                    },
                    subject: ChangeSubject::Profile {
                        target: plan.target.id.clone(),
                        profile: profile.package.name.clone(),
                    },
                });
            }
        }
        changes.sort_by(|left, right| {
            change_target(&left.subject)
                .cmp(change_target(&right.subject))
                .then(left.subject.cmp(&right.subject))
                .then(left.kind.cmp(&right.kind))
        });
        UnpackPreview { changes }
    }
}

fn change_target(subject: &ChangeSubject) -> &str {
    match subject {
        ChangeSubject::TargetDefinition { target } | ChangeSubject::Profile { target, .. } => {
            target
        }
    }
}

struct TargetPlan {
    target: Arc<TargetSpec>,
    profiles: Vec<PlannedProfile>,
    unchanged: usize,
    skipped: usize,
}

struct PlannedProfile {
    package: PackageProfile,
    overwrite: bool,
}

struct ProfileCandidate {
    package: PackageProfile,
    exists: bool,
}

impl TargetPlan {
    fn prepare(
        target: Arc<TargetSpec>,
        profiles: Vec<PackageProfile>,
        interaction: &mut impl UnpackInteraction,
    ) -> Result<Self> {
        let mut unchanged = 0;
        let mut candidates = Vec::with_capacity(profiles.len());
        for package_profile in profiles {
            let directory = config::profile_dir(&target, &package_profile.name)?;
            let exists = directory.is_dir();
            let changed = !exists || profile_changed(&target, &package_profile, &directory)?;
            if !changed {
                unchanged += 1;
                continue;
            }
            candidates.push(ProfileCandidate {
                package: package_profile,
                exists,
            });
        }
        if !candidates.is_empty() {
            interaction.begin_target(&target.id);
        }

        let mut planned = Vec::with_capacity(candidates.len());
        let mut skipped = 0;
        for candidate in candidates {
            if candidate.exists && !interaction.overwrite_profile(&candidate.package.name)? {
                skipped += 1;
                continue;
            }
            storage::validate_replacement(
                &target,
                &candidate.package.name,
                &candidate.package.resources,
                candidate.exists,
            )?;
            planned.push(PlannedProfile {
                package: candidate.package,
                overwrite: candidate.exists,
            });
        }
        Ok(Self {
            target,
            profiles: planned,
            unchanged,
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
            unchanged: self.unchanged,
            skipped: self.skipped,
        }
    }
}

fn profile_changed(
    target: &TargetSpec,
    package: &PackageProfile,
    directory: &Path,
) -> Result<bool> {
    let mut expected = HashSet::new();
    for resource in &package.resources {
        expected.insert(resource.spec.filename.clone());
        let path = match config::profile_resource(target, &package.name, &resource.spec) {
            // The normal unpack replaces the whole profile directory, so an
            // existing resource symlink is itself a modification to preview.
            Err(AppError::Path(PathError::Unsafe(_))) => return Ok(true),
            result => result?,
        };
        match fs::read(&path) {
            Ok(content) if content == resource.content => {}
            Ok(_) => return Ok(true),
            Err(_) => return Ok(true),
        }
    }

    for entry in fs::read_dir(directory).with_path(directory)? {
        let entry = entry.with_path(directory)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !expected.contains(&name) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn target_config_changes(
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

fn selected_packages(
    targets: &TargetRepository,
    path: &Path,
    target_id: Option<&str>,
) -> Result<Vec<TargetPackage>> {
    let packages = read_package(targets, path)?;
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
