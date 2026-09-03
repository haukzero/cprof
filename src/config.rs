use std::path::PathBuf;

use crate::error::{AppError, Result};
use crate::targets::{ResourceSpec, TargetSpec};

pub fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or(AppError::NoHomeDir)
}

pub fn repository_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".cprof"))
}

pub fn profiles_root() -> Result<PathBuf> {
    Ok(repository_dir()?.join("profiles"))
}

pub fn profiles_dir(target: &TargetSpec) -> Result<PathBuf> {
    Ok(profiles_root()?.join(target.id))
}

pub fn profile_dir(target: &TargetSpec, name: &str) -> Result<PathBuf> {
    Ok(profiles_dir(target)?.join(name))
}

pub fn profile_resource(
    target: &TargetSpec,
    name: &str,
    resource: &ResourceSpec,
) -> Result<PathBuf> {
    Ok(profile_dir(target, name)?.join(resource.filename))
}

pub fn active_resource(target: &TargetSpec, resource: &ResourceSpec) -> Result<PathBuf> {
    Ok(target.active_path(&home_dir()?, resource))
}
