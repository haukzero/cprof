use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config;
use crate::error::{AppError, Result};
use crate::fs_util;
use crate::targets::{ResourceSpec, TargetSpec};

#[derive(Debug, Clone)]
pub struct ProfileInfo {
    pub name: String,
    pub complete: bool,
}

#[derive(Debug, Clone)]
pub struct ProfileResource {
    pub spec: &'static ResourceSpec,
    pub content: Vec<u8>,
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.contains(['/', '\\'])
        || name.chars().any(char::is_control)
    {
        return Err(AppError::InvalidProfileName(name.to_string()));
    }
    Ok(())
}

pub fn exists(target: &TargetSpec, name: &str) -> Result<bool> {
    validate_name(name)?;
    Ok(config::profile_dir(target, name)?.is_dir())
}

pub fn is_complete(target: &TargetSpec, name: &str) -> Result<bool> {
    validate_name(name)?;
    let dir = config::profile_dir(target, name)?;
    if !dir.is_dir() {
        return Ok(false);
    }

    for spec in target.resources {
        let path = dir.join(spec.filename);
        if !path.is_file() {
            if spec.required {
                return Ok(false);
            }
            continue;
        }
        if (spec.validate)(&fs::read(path)?).is_err() {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn list(target: &TargetSpec) -> Result<Vec<ProfileInfo>> {
    let dir = config::profiles_dir(target)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut profiles = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if validate_name(&name).is_ok() {
            profiles.push(ProfileInfo {
                complete: is_complete(target, &name)?,
                name,
            });
        }
    }
    profiles.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    Ok(profiles)
}

pub fn create(target: &TargetSpec, name: &str, copy_from: Option<&str>) -> Result<()> {
    validate_name(name)?;
    let dir = config::profile_dir(target, name)?;
    if dir.exists() {
        return Err(AppError::ProfileExists(name.to_string()));
    }

    let source_dir = match copy_from {
        Some(source) => {
            if !exists(target, source)? {
                return Err(AppError::ProfileNotFound(source.to_string()));
            }
            Some(config::profile_dir(target, source)?)
        }
        None => None,
    };

    fs::create_dir_all(&dir)?;
    let result = target.resources.iter().try_for_each(|resource| {
        let destination = dir.join(resource.filename);
        if let Some(source_dir) = &source_dir {
            let source_path = source_dir.join(resource.filename);
            if source_path.exists() {
                fs_util::write_file(&destination, &fs::read(source_path)?)?;
            }
        } else {
            write_resource(&destination, resource, resource.template)?;
        }
        Ok::<(), AppError>(())
    });
    if let Err(error) = result {
        let _ = fs_util::remove_dir_if_exists(&dir);
        return Err(error);
    }
    Ok(())
}

pub fn delete(target: &TargetSpec, name: &str) -> Result<()> {
    validate_name(name)?;
    let dir = config::profile_dir(target, name)?;
    if !dir.is_dir() {
        return Err(AppError::ProfileNotFound(name.to_string()));
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

pub fn replace(
    target: &TargetSpec,
    name: &str,
    resources: &[ProfileResource],
    overwrite: bool,
) -> Result<()> {
    validate_name(name)?;
    let destination = config::profile_dir(target, name)?;
    if destination.exists() && !overwrite {
        return Err(AppError::ProfileExists(name.to_string()));
    }
    validate_resources(target, name, resources)?;

    let staging_dir =
        config::profiles_dir(target)?.join(format!(".{name}.incoming-{}", std::process::id()));
    let rollback_dir = staging_dir.with_extension("rollback");
    fs_util::remove_dir_if_exists(&staging_dir)?;
    fs_util::remove_dir_if_exists(&rollback_dir)?;
    fs::create_dir_all(&staging_dir)?;

    if let Err(error) = write_resources(&staging_dir, resources) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(error);
    }

    if overwrite {
        fs::rename(&destination, &rollback_dir)?;
    }
    if let Err(error) = fs::rename(&staging_dir, &destination) {
        if overwrite {
            let _ = fs::rename(&rollback_dir, &destination);
        }
        return Err(error.into());
    }
    let _ = fs_util::remove_dir_if_exists(&rollback_dir);
    Ok(())
}

pub fn read(target: &'static TargetSpec, name: &str) -> Result<Vec<ProfileResource>> {
    validate_name(name)?;
    if !exists(target, name)? {
        return Err(AppError::ProfileNotFound(name.to_string()));
    }

    let mut resources = Vec::new();
    for spec in target.resources {
        let path = config::profile_resource(target, name, spec)?;
        if !path.exists() {
            if spec.required {
                return Err(AppError::IncompleteProfile(name.to_string()));
            }
            continue;
        }
        let content = fs::read(path)?;
        (spec.validate)(&content)?;
        resources.push(ProfileResource { spec, content });
    }
    Ok(resources)
}

pub fn resource_path(target: &TargetSpec, name: &str, resource: &ResourceSpec) -> Result<PathBuf> {
    validate_name(name)?;
    config::profile_resource(target, name, resource)
}

pub fn validate_resource_file(path: &Path, resource: &ResourceSpec) -> Result<()> {
    (resource.validate)(&fs::read(path)?)?;
    Ok(())
}

fn write_resources(dir: &Path, resources: &[ProfileResource]) -> Result<()> {
    for resource in resources {
        let path = dir.join(resource.spec.filename);
        fs_util::write_file(&path, &resource.content)?;
    }
    Ok(())
}

fn validate_resources(
    target: &TargetSpec,
    name: &str,
    resources: &[ProfileResource],
) -> Result<()> {
    let mut keys = HashSet::new();
    for resource in resources {
        let expected = target.resource(resource.spec.key)?;
        if expected.filename != resource.spec.filename || !keys.insert(resource.spec.key) {
            return Err(AppError::InvalidResource(format!(
                "Invalid resource '{}' in profile '{name}'",
                resource.spec.key
            )));
        }
        (expected.validate)(&resource.content)?;
    }
    if target
        .resources
        .iter()
        .any(|spec| spec.required && !keys.contains(spec.key))
    {
        return Err(AppError::IncompleteProfile(name.to_string()));
    }
    Ok(())
}

fn write_resource(path: &Path, spec: &ResourceSpec, content: &[u8]) -> Result<()> {
    (spec.validate)(content)?;
    fs_util::write_file(path, content)
}
