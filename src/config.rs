use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};
use crate::paths;
use crate::targets::{ResourceSpec, TargetSpec};

pub fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or(AppError::NoHomeDir)
}

pub fn repository_dir() -> Result<PathBuf> {
    paths::join_under(&home_dir()?, Path::new(".cprof"))
}

pub fn profiles_root() -> Result<PathBuf> {
    paths::join_under(&repository_dir()?, Path::new("profiles"))
}

pub(crate) fn extra_target_file() -> Result<PathBuf> {
    paths::join_storage_under(
        &repository_dir()?,
        Path::new(crate::targets::EXTRA_TARGET_FILE),
    )
}

pub fn profiles_dir(target: &TargetSpec) -> Result<PathBuf> {
    paths::validate_target_id(target.id)?;
    paths::join_storage_under(&profiles_root()?, Path::new(target.id))
}

pub fn profile_dir(target: &TargetSpec, name: &str) -> Result<PathBuf> {
    crate::profile::validate_name(name)?;
    paths::join_storage_under(&profiles_dir(target)?, Path::new(name))
}

pub fn profile_resource(
    target: &TargetSpec,
    name: &str,
    resource: &ResourceSpec,
) -> Result<PathBuf> {
    paths::validate_filename(resource.filename)?;
    paths::join_storage_under(&profile_dir(target, name)?, Path::new(resource.filename))
}

pub fn active_resource(target: &TargetSpec, resource: &ResourceSpec) -> Result<PathBuf> {
    target.active_path(&home_dir()?, resource)
}
