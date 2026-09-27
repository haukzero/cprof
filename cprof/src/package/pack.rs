//! Collect stored profiles and atomically write a portable package.

use std::path::Path;

use crate::error::Result;
use crate::filesystem;
use crate::profile::activation;
use crate::profile::storage;
use crate::targets::{TargetRepository, TargetSpec};

use super::{PackageProfile, TargetPackage, encode_with_repository};

pub(crate) struct PackReport {
    pub(crate) target_count: usize,
    pub(crate) profile_count: usize,
}

pub(crate) fn create<'a>(
    targets: &TargetRepository,
    selected: impl IntoIterator<Item = &'a TargetSpec>,
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

fn collect_target(target: &TargetSpec) -> Result<Option<TargetPackage>> {
    let names = storage::names(target)?;
    if names.is_empty() {
        return Ok(None);
    }
    let mut profiles = Vec::with_capacity(names.len());
    for name in names {
        let resources = storage::read(target, &name)?;
        profiles.push(PackageProfile::new(name, resources));
    }
    Ok(Some(TargetPackage::with_active(
        target.id.clone(),
        profiles,
        activation::active_name(target)?,
    )))
}
