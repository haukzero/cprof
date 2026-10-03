//! Collect stored profiles and atomically write a portable package.

use std::path::Path;
use std::sync::Arc;

use indexmap::IndexSet;

use crate::error::Result;
use crate::filesystem;
use crate::profile::{ProfileInfo, activation, storage};
use crate::targets::{TargetRepository, TargetSpec};

use super::{PackageProfile, TargetPackage, encode_with_repository};

pub(crate) struct PackReport {
    pub(crate) target_count: usize,
    pub(crate) profile_count: usize,
}

/// A resolved selection: only these stored profiles may enter the package.
pub(crate) struct SelectedTarget {
    pub(crate) target: Arc<TargetSpec>,
    pub(crate) profiles: IndexSet<String>,
}

pub(crate) fn create(
    targets: &TargetRepository,
    selected: &[SelectedTarget],
    output: &Path,
) -> Result<PackReport> {
    let mut packages = Vec::new();
    for target in selected {
        if let Some(package) = collect_target(target)? {
            packages.push(package);
        }
    }
    // Encoding also rejects an empty collection before the output is touched.
    let data = encode_with_repository(targets, &packages)?;
    filesystem::atomic_write(output, &data)?;
    Ok(PackReport {
        target_count: packages.len(),
        profile_count: packages.iter().map(|package| package.profiles.len()).sum(),
    })
}

fn collect_target(selected: &SelectedTarget) -> Result<Option<TargetPackage>> {
    let target = &selected.target;
    if selected.profiles.is_empty() {
        return Ok(None);
    }
    let mut profiles = Vec::with_capacity(selected.profiles.len());
    for name in &selected.profiles {
        let resources = storage::read(target, name)?;
        profiles.push(PackageProfile::new(name.clone(), resources));
    }
    // These profiles have already been validated. Do not inspect the contents
    // of an excluded active profile just to determine package metadata.
    let complete_profiles = profiles
        .iter()
        .map(|profile| ProfileInfo {
            name: profile.name.clone(),
            complete: true,
        })
        .collect::<Vec<_>>();
    Ok(Some(TargetPackage::with_active(
        target.id.clone(),
        profiles,
        activation::active_name_from_profiles(target, &complete_profiles)?,
    )))
}
