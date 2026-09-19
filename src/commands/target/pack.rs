use std::path::Path;

use crate::error::{AppError, Result};
use crate::fs_util;
use crate::package;
use crate::profile;
use crate::targets::TargetSpec;

pub fn run(target: &'static TargetSpec, save: Option<String>) -> Result<()> {
    let output_path = save.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let output = Path::new(&output_path);
    let package = collect_target(target)?
        .ok_or_else(|| AppError::InvalidPackage("No profiles to pack".to_string()))?;
    write_package(output, &[package])
}

pub(crate) fn collect_target(
    target: &'static TargetSpec,
) -> Result<Option<package::TargetPackage>> {
    let profiles = profile::list(target)?;
    if profiles.is_empty() {
        return Ok(None);
    }
    let mut entries = Vec::new();
    for profile_info in profiles {
        let resources = profile::read(target, &profile_info.name)?;
        entries.push(package::PackageProfile::new(profile_info.name, resources));
    }
    Ok(Some(package::TargetPackage::new(target.id, entries)))
}

pub(crate) fn write_package(output: &Path, packages: &[package::TargetPackage]) -> Result<()> {
    let data = package::encode(packages)?;
    fs_util::atomic_write(output, &data)?;
    let profile_count = packages
        .iter()
        .map(|package| package.profiles.len())
        .sum::<usize>();
    println!(
        "Packed {profile_count} profile(s) from {} target(s) to '{}'",
        packages.len(),
        output.display()
    );
    Ok(())
}
