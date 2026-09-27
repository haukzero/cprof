use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::config;
use crate::error::{AppError, IoContext, PackageError, PathError, Result};
use crate::filesystem::transaction::PathTransaction;
use crate::package::PackageProfile;
use crate::profile::{self, activation, storage};
use crate::targets::TargetSpec;

use super::{ActiveChange, UnpackInteraction, UnpackMode, UnpackedTarget};

pub(super) struct TargetPlan {
    pub(super) target: Arc<TargetSpec>,
    pub(super) profiles: Vec<PlannedProfile>,
    pub(super) removed: Vec<String>,
    pub(super) active_change: Option<ActiveChange>,
    active_resources: Option<BTreeSet<String>>,
    unchanged: usize,
    skipped: usize,
}

pub(super) struct PlannedProfile {
    pub(super) package: PackageProfile,
    pub(super) overwrite: bool,
}

struct ProfileCandidate {
    package: PackageProfile,
    exists: bool,
}

impl TargetPlan {
    pub(super) fn prepare(
        target: Arc<TargetSpec>,
        profiles: Vec<PackageProfile>,
        desired_active: Option<String>,
        active_profile_known: bool,
        mode: UnpackMode,
        interaction: &mut impl UnpackInteraction,
    ) -> Result<Self> {
        let mut unchanged = 0;
        if mode == UnpackMode::Mirror
            && let Some(active_profile) = desired_active.as_deref()
            && !profiles
                .iter()
                .any(|profile| profile.name == active_profile)
        {
            return Err(PackageError::Invalid(format!(
                "Active profile '{active_profile}' is not included for target '{}'",
                target.id
            ))
            .into());
        }
        let package_names = (mode == UnpackMode::Mirror).then(|| {
            profiles
                .iter()
                .map(|profile| profile.name.clone())
                .collect::<BTreeSet<_>>()
        });
        let mut candidates = Vec::with_capacity(profiles.len());
        for package_profile in &profiles {
            let directory = config::profile_dir(&target, &package_profile.name)?;
            let exists = directory.is_dir();
            let changed = !exists || profile_changed(&target, package_profile, &directory)?;
            if !changed {
                unchanged += 1;
                continue;
            }
            candidates.push(ProfileCandidate {
                package: package_profile.clone(),
                exists,
            });
        }
        let removed_candidates = if mode == UnpackMode::Mirror {
            storage::names(&target)?
                .into_iter()
                .filter(|name| {
                    !package_names
                        .as_ref()
                        .expect("mirror package names")
                        .contains(name)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let active_change = if mode == UnpackMode::Mirror && active_profile_known {
            let status = activation::status(&target)?;
            let previous = status.active_name().map(str::to_owned);
            let changed = match desired_active.as_deref() {
                Some(name) => previous.as_deref() != Some(name),
                None => !matches!(status, activation::Status::NoFiles),
            };
            changed.then_some(ActiveChange {
                previous,
                desired: desired_active.clone(),
            })
        } else if mode == UnpackMode::Mirror {
            let status = activation::status(&target)?;
            let previous = status.active_name().map(str::to_owned);
            let removed_active = previous
                .as_ref()
                .is_some_and(|name| removed_candidates.contains(name));
            removed_active.then_some(ActiveChange {
                previous,
                desired: None,
            })
        } else {
            None
        };
        let active_resources = active_profile_known
            .then_some(desired_active.as_ref())
            .flatten()
            .and_then(|active| {
                profiles
                    .iter()
                    .find(|profile| &profile.name == active)
                    .map(|profile| {
                        profile
                            .resources
                            .iter()
                            .map(|resource| resource.spec.key.clone())
                            .collect()
                    })
            });
        if !candidates.is_empty() || !removed_candidates.is_empty() || active_change.is_some() {
            interaction.begin_target(&target.id);
        }

        let mut planned = Vec::with_capacity(candidates.len());
        let mut skipped = 0;
        for candidate in candidates {
            if candidate.exists
                && !interaction.overwrite_profile(&target.id, &candidate.package.name)?
            {
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
        let mut removed = Vec::with_capacity(removed_candidates.len());
        for name in removed_candidates {
            if interaction.remove_profile(&target.id, &name)? {
                removed.push(name);
            }
        }
        Ok(Self {
            target,
            profiles: planned,
            removed,
            active_change,
            active_resources,
            unchanged,
            skipped,
        })
    }

    pub(super) fn stage(&self, transaction: &mut PathTransaction) -> Result<()> {
        for planned in &self.profiles {
            storage::stage_validated_replace(
                transaction,
                &self.target,
                &planned.package.name,
                &planned.package.resources,
                planned.overwrite,
            )?;
        }
        for name in &self.removed {
            storage::stage_delete(transaction, &self.target, name)?;
        }
        if let Some(active_change) = &self.active_change {
            activation::stage_switch(
                transaction,
                &self.target,
                active_change.desired.as_deref(),
                true,
                self.active_resources.as_ref(),
            )?;
        }
        Ok(())
    }

    pub(super) fn into_report(self) -> UnpackedTarget {
        UnpackedTarget {
            target: self.target.id.clone(),
            profiles: self
                .profiles
                .into_iter()
                .map(|profile| profile.package.name)
                .collect(),
            removed: self.removed,
            active_change: self.active_change,
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

pub(super) fn remap_profiles(
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
