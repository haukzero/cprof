use std::fs;
use std::path::Path;

use dialoguer::Confirm;

use crate::error::{AppError, Result};
use crate::package;
use crate::profile;
use crate::style;
use crate::targets::{self, TargetSpec};

pub fn run(target: &'static TargetSpec, path: Option<String>, force: bool) -> Result<()> {
    let package_path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let package_path = Path::new(&package_path);
    if !package_path.exists() {
        return Err(AppError::Other(format!(
            "Package file '{}' not found",
            package_path.display()
        )));
    }

    let packages = package::decode(&fs::read(package_path)?)?;
    let package = packages
        .into_iter()
        .find(|package| package.target == target.id)
        .ok_or_else(|| {
            AppError::InvalidPackage(format!("Package does not contain target '{}'", target.id))
        })?;
    unpack_target(target, package.profiles, force).map(|_| ())
}

pub fn run_all(path: Option<String>, force: bool) -> Result<()> {
    let package_path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let package_path = Path::new(&package_path);
    if !package_path.exists() {
        return Err(AppError::Other(format!(
            "Package file '{}' not found",
            package_path.display()
        )));
    }
    let packages = package::decode(&fs::read(package_path)?)?;
    let mut targets = 0;
    let mut unpacked = 0;
    let mut skipped = 0;
    for package in packages {
        let target = targets::get(&package.target)?;
        let (written, ignored) = unpack_target(target, package.profiles, force)?;
        targets += 1;
        unpacked += written;
        skipped += ignored;
    }
    println!("\nDone: {unpacked} unpacked, {skipped} skipped across {targets} target(s)");
    Ok(())
}

fn unpack_target(
    target: &'static TargetSpec,
    profiles: Vec<package::PackageProfile>,
    force: bool,
) -> Result<(usize, usize)> {
    let mut unpacked = 0;
    let mut skipped = 0;
    for package_profile in profiles {
        let exists = profile::exists(target, &package_profile.name)?;
        if exists {
            println!(
                "{}",
                style::warning(&format!(
                    "Profile '{}' already exists - conflict!",
                    package_profile.name
                ))
            );
            let overwrite = force || should_overwrite(&package_profile.name)?;
            if !overwrite {
                println!("Skipped '{}'", package_profile.name);
                skipped += 1;
                continue;
            }
        }

        profile::replace(
            target,
            &package_profile.name,
            &package_profile.resources,
            exists,
        )?;
        println!("Unpacked '{}'", package_profile.name);
        unpacked += 1;
    }
    println!("Unpacked target '{}'", target.id);
    Ok((unpacked, skipped))
}

fn should_overwrite(name: &str) -> Result<bool> {
    if !atty::is(atty::Stream::Stdin) {
        return Ok(false);
    }
    Confirm::new()
        .with_prompt(format!("Overwrite '{name}' ?"))
        .default(false)
        .interact()
        .map_err(|error| AppError::Other(error.to_string()))
}
