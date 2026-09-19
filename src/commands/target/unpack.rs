use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::error::{AppError, IoContext, Result};
use crate::fs_util::PathTransaction;
use crate::package;
use crate::profile;
use crate::prompt;
use crate::style;
use crate::targets::{self, ExternalTargetConfig, TargetRepository, TargetSpec};

pub fn run(
    targets: &TargetRepository,
    target: &TargetSpec,
    path: Option<String>,
    force: bool,
) -> Result<()> {
    let packages = read_package(targets, path)?;
    let package = packages
        .into_iter()
        .find(|package| package.target == target.id)
        .ok_or_else(|| {
            AppError::InvalidPackage(format!("Package does not contain target '{}'", target.id))
        })?;
    let (target_configs, configs_changed) =
        merge_target_configs(targets, std::slice::from_ref(&package), force)?;
    let target = resolve_target(targets, &package, &target_configs)?;
    let profiles = remap_profiles(&target, package.profiles)?;
    let plan = prepare_unpack(target, profiles, force)?;
    commit_unpack(
        std::slice::from_ref(&plan),
        &target_configs,
        configs_changed,
    )?;
    plan.report();
    Ok(())
}

pub(crate) struct UnpackPlan {
    target: Arc<TargetSpec>,
    profiles: Vec<PlannedProfile>,
    skipped: usize,
}

struct PlannedProfile {
    package: package::PackageProfile,
    overwrite: bool,
}

impl UnpackPlan {
    pub(crate) fn stage(&self, transaction: &mut PathTransaction) -> Result<()> {
        for planned in &self.profiles {
            profile::stage_validated_replace(
                transaction,
                &self.target,
                &planned.package.name,
                &planned.package.resources,
                planned.overwrite,
            )?;
        }
        Ok(())
    }

    pub(crate) fn report(&self) -> (usize, usize) {
        for planned in &self.profiles {
            println!("Unpacked '{}'", planned.package.name);
        }
        println!(
            "{}",
            style::heading(&format!("Unpacked target '{}'", self.target.id))
        );
        (self.profiles.len(), self.skipped)
    }
}

pub(crate) fn prepare_unpack(
    target: Arc<TargetSpec>,
    profiles: Vec<package::PackageProfile>,
    force: bool,
) -> Result<UnpackPlan> {
    println!(
        "{}",
        style::heading(&format!("Unpacking target '{}':", target.id))
    );
    let mut planned = Vec::with_capacity(profiles.len());
    let mut skipped = 0;
    for package_profile in profiles {
        let exists = profile::exists(&target, &package_profile.name)?;
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
        profile::validate_replacement(
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
    Ok(UnpackPlan {
        target,
        profiles: planned,
        skipped,
    })
}

pub(crate) fn commit_unpack(
    plans: &[UnpackPlan],
    target_configs: &BTreeMap<String, ExternalTargetConfig>,
    configs_changed: bool,
) -> Result<()> {
    let mut transaction = PathTransaction::new();
    for plan in plans {
        if let Err(error) = plan.stage(&mut transaction) {
            return Err(transaction.cancel(error));
        }
    }
    if configs_changed
        && let Err(error) = targets::stage_external_configs(&mut transaction, target_configs)
    {
        return Err(transaction.cancel(error));
    }
    transaction.commit()
}

pub(crate) fn merge_target_configs(
    targets: &TargetRepository,
    packages: &[package::TargetPackage],
    force: bool,
) -> Result<(BTreeMap<String, ExternalTargetConfig>, bool)> {
    let packaged = packages
        .iter()
        .filter_map(|package| package.target_config.clone())
        .collect::<Vec<_>>();
    let local = targets.external_configs().clone();
    let merged = targets::merge_external_configs(local.clone(), &packaged, |prompt| {
        should_use_packaged(prompt, force)
    })?;
    let changed = merged != local;
    Ok((merged, changed))
}

pub(crate) fn read_package(
    targets: &TargetRepository,
    path: Option<String>,
) -> Result<Vec<package::TargetPackage>> {
    let path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let path = Path::new(&path);
    if !path.exists() {
        return Err(AppError::Other(format!(
            "Package file '{}' not found",
            path.display()
        )));
    }
    package::decode_with_repository(targets, &fs::read(path).with_path(path)?)
}

fn effective_target_from_config(
    package: &package::TargetPackage,
    configs: &std::collections::BTreeMap<String, ExternalTargetConfig>,
) -> Result<Arc<TargetSpec>> {
    let config = targets::find_external_config(configs, &package.target).ok_or_else(|| {
        AppError::InvalidPackage(format!(
            "Merged configuration does not contain target '{}'",
            package.target
        ))
    })?;
    Ok(Arc::new(targets::spec_from_external_config(config)?))
}

pub(crate) fn resolve_target(
    targets: &TargetRepository,
    package: &package::TargetPackage,
    configs: &std::collections::BTreeMap<String, ExternalTargetConfig>,
) -> Result<Arc<TargetSpec>> {
    if package.target_config.is_some() {
        effective_target_from_config(package, configs)
    } else {
        targets.get(&package.target)
    }
}

pub(crate) fn remap_profiles(
    target: &TargetSpec,
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
