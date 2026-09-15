use std::fs;
use std::path::Path;

use crate::error::{AppError, Result};
use crate::package;
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::{self, ExternalTargetConfig, TargetSpec};

pub fn run(target: &'static TargetSpec, path: Option<String>, force: bool) -> Result<()> {
    let packages = read_package(path)?;
    let package = packages
        .into_iter()
        .find(|package| package.target == target.id)
        .ok_or_else(|| {
            AppError::InvalidPackage(format!("Package does not contain target '{}'", target.id))
        })?;
    let target_configs = merge_target_configs(std::slice::from_ref(&package), force)?;
    let target = resolve_target(&package, &target_configs)?;
    let profiles = remap_profiles(target, package.profiles)?;
    unpack_target(target, profiles, force)?;
    Ok(())
}

pub(crate) fn unpack_target(
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
    println!(
        "{}",
        style::heading(&format!("Unpacked target '{}'", target.id))
    );
    Ok((unpacked, skipped))
}

pub(crate) fn merge_target_configs(
    packages: &[package::TargetPackage],
    force: bool,
) -> Result<std::collections::BTreeMap<String, ExternalTargetConfig>> {
    let packaged = packages
        .iter()
        .filter_map(|package| package.target_config.clone())
        .collect::<Vec<_>>();
    let local = targets::read_external_configs()?;
    let merged = targets::merge_external_configs(local.clone(), &packaged, |prompt| {
        should_use_packaged(prompt, force)
    })?;
    if merged != local {
        targets::write_external_configs(&merged)?;
    }
    Ok(merged)
}

pub(crate) fn read_package(path: Option<String>) -> Result<Vec<package::TargetPackage>> {
    let path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let path = Path::new(&path);
    if !path.exists() {
        return Err(AppError::Other(format!(
            "Package file '{}' not found",
            path.display()
        )));
    }
    package::decode(&fs::read(path)?)
}

fn effective_target_from_config(
    package: &package::TargetPackage,
    configs: &std::collections::BTreeMap<String, ExternalTargetConfig>,
) -> Result<&'static TargetSpec> {
    let config = targets::find_external_config(configs, &package.target).ok_or_else(|| {
        AppError::InvalidPackage(format!(
            "Merged configuration does not contain target '{}'",
            package.target
        ))
    })?;
    Ok(Box::leak(Box::new(targets::spec_from_external_config(
        config,
    )?)))
}

pub(crate) fn resolve_target(
    package: &package::TargetPackage,
    configs: &std::collections::BTreeMap<String, ExternalTargetConfig>,
) -> Result<&'static TargetSpec> {
    if package.target_config.is_some() {
        effective_target_from_config(package, configs)
    } else {
        targets::get(&package.target)
    }
}

pub(crate) fn remap_profiles(
    target: &'static TargetSpec,
    profiles: Vec<package::PackageProfile>,
) -> Result<Vec<package::PackageProfile>> {
    profiles
        .into_iter()
        .map(|profile| {
            let mut present = std::collections::HashSet::new();
            let mut resources = profile
                .resources
                .into_iter()
                .map(|resource| {
                    let spec = target.resource(resource.spec.key)?;
                    present.insert(spec.key);
                    Ok(profile::ProfileResource {
                        spec,
                        content: resource.content,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            for spec in target.resources {
                if spec.required && !present.contains(spec.key) {
                    resources.push(profile::ProfileResource {
                        spec,
                        content: spec.template.to_vec(),
                    });
                }
            }
            Ok(package::PackageProfile::new(profile.name, resources))
        })
        .collect()
}

fn should_overwrite(name: &str) -> Result<bool> {
    prompt::confirm(&format!("Overwrite '{name}' ?"))
}

fn should_use_packaged(prompt: &str, force: bool) -> Result<bool> {
    if force {
        return Ok(true);
    }
    prompt::confirm(prompt)
}
