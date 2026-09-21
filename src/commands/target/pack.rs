use std::path::Path;

use crate::error::{AppError, Result};
use crate::filesystem;
use crate::package;
use crate::profile::storage;
use crate::targets::{TargetRepository, TargetSpec};

pub fn run(targets: &TargetRepository, target: &TargetSpec, save: Option<String>) -> Result<()> {
    let output_path = save.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let output = Path::new(&output_path);
    let package = collect_target(target)?
        .ok_or_else(|| AppError::InvalidPackage("No profiles to pack".to_string()))?;
    write_package(targets, output, &[package])
}

pub(crate) fn collect_target(target: &TargetSpec) -> Result<Option<package::TargetPackage>> {
    let names = storage::names(target)?;
    if names.is_empty() {
        return Ok(None);
    }
    let mut entries = Vec::with_capacity(names.len());
    for name in names {
        let resources = storage::read(target, &name)?;
        entries.push(package::PackageProfile::new(name, resources));
    }
    Ok(Some(package::TargetPackage::new(
        target.id.clone(),
        entries,
    )))
}

pub(crate) fn write_package(
    targets: &TargetRepository,
    output: &Path,
    packages: &[package::TargetPackage],
) -> Result<()> {
    let data = package::encode_with_repository(targets, packages)?;
    filesystem::atomic_write(output, &data)?;
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
