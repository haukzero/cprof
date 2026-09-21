//! Inspect and switch active resource links for stored profiles.

use std::fs;
use std::io;
use std::path::Path;

use crate::config::{self, paths};
use crate::error::{AppError, IoContext, Result};
use crate::filesystem::{self, transaction::PathTransaction};
use crate::targets::{ResourceSpec, TargetSpec};

use super::{ProfileInfo, storage, validate_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// All stored resources are linked to one complete cprof profile.
    Active(String),
    /// None of the target's active resource paths exists.
    NoFiles,
    /// Managed links are missing or broken, or the profile fails validation.
    Partial,
    /// Different resources point to different cprof profiles.
    Mixed,
    /// At least one entry is a file or link not recognized as a managed resource.
    Unmanaged,
}

impl Status {
    pub fn active_name(&self) -> Option<&str> {
        match self {
            Self::Active(name) => Some(name),
            _ => None,
        }
    }

    pub(crate) fn display_name(&self) -> &str {
        match self {
            Self::Active(name) => name,
            Self::NoFiles => "(none)",
            Self::Partial => "(partial)",
            Self::Mixed => "(mixed)",
            Self::Unmanaged => "(unmanaged)",
        }
    }
}

pub fn status(target: &TargetSpec) -> Result<Status> {
    status_with(target, |name| storage::is_complete(target, name))
}

fn status_with(
    target: &TargetSpec,
    is_complete: impl FnOnce(&str) -> Result<bool>,
) -> Result<Status> {
    let mut linked_profiles = Vec::new();

    for resource in &target.resources {
        let link = config::active_resource(target, resource)?;
        let metadata = match fs::symlink_metadata(&link) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(AppError::io(&link, error)),
        };
        if !metadata.file_type().is_symlink() {
            return Ok(Status::Unmanaged);
        }
        match managed_link_profile(target, resource, &link)? {
            Some(name) => linked_profiles.push(name),
            None => return Ok(Status::Unmanaged),
        }
    }

    let Some(name) = linked_profiles.first() else {
        return Ok(Status::NoFiles);
    };
    if linked_profiles.iter().any(|linked| linked != name) {
        return Ok(Status::Mixed);
    }

    let mut stored_resources = 0;
    for resource in &target.resources {
        if config::profile_resource(target, name, resource)?.exists() {
            stored_resources += 1;
        }
    }
    if linked_profiles.len() != stored_resources || !is_complete(name)? {
        return Ok(Status::Partial);
    }
    Ok(Status::Active(name.clone()))
}

pub fn active_name(target: &TargetSpec) -> Result<Option<String>> {
    Ok(status(target)?.active_name().map(str::to_owned))
}

pub(crate) fn active_name_from_profiles(
    target: &TargetSpec,
    profiles: &[ProfileInfo],
) -> Result<Option<String>> {
    let status = status_with(target, |name| {
        Ok(profiles
            .iter()
            .find(|profile| profile.name == name)
            .is_some_and(|profile| profile.complete))
    })?;
    Ok(status.active_name().map(str::to_owned))
}

pub fn switch(target: &TargetSpec, name: &str, force: bool) -> Result<bool> {
    validate_name(name)?;
    if !storage::is_complete(target, name)? {
        return Err(AppError::IncompleteProfile(name.to_string()));
    }
    if active_name(target)?.as_deref() == Some(name) {
        return Ok(true);
    }

    PathTransaction::prepare(|transaction| {
        for resource in &target.resources {
            let link = config::active_resource(target, resource)?;
            let source = config::profile_resource(target, name, resource)?;
            let exists = filesystem::path_exists(&link)?;
            if exists && !force && managed_link_profile(target, resource, &link)?.is_none() {
                return Err(AppError::UnmanagedActivePath(link.display().to_string()));
            }
            if source.exists() {
                transaction.stage_symlink(&link, &source, exists)?;
            } else if exists {
                transaction.stage_remove(&link)?;
            }
        }
        Ok(())
    })?
    .commit()?;
    Ok(false)
}

pub fn remove_profile_links(target: &TargetSpec, name: &str) -> Result<bool> {
    let mut removed = false;
    for resource in &target.resources {
        let link = config::active_resource(target, resource)?;
        if managed_link_profile(target, resource, &link)?.as_deref() == Some(name) {
            fs::remove_file(&link).with_path(&link)?;
            removed = true;
        }
    }
    Ok(removed)
}

fn managed_link_profile(
    target: &TargetSpec,
    resource: &ResourceSpec,
    link: &Path,
) -> Result<Option<String>> {
    let metadata = match fs::symlink_metadata(link) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AppError::io(link, error)),
    };
    if !metadata.file_type().is_symlink() {
        return Ok(None);
    }

    let raw_target = fs::read_link(link).with_path(link)?;
    let target_path = if raw_target.is_absolute() {
        raw_target
    } else {
        link.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(raw_target)
    };
    // Resolve parent aliases and relative components without following the
    // resource leaf: a missing stored file is still a managed (partial) link.
    // Canonicalizing both parents also handles Windows verbatim path prefixes.
    let target_path = paths::path_identity(&target_path)?;
    let profiles_dir = paths::resolve_existing(&config::profiles_dir(target)?)?;
    let Ok(relative) = target_path.strip_prefix(&profiles_dir) else {
        return Ok(None);
    };
    let mut parts = relative.components();
    let (Some(profile_name), Some(filename), None) = (parts.next(), parts.next(), parts.next())
    else {
        return Ok(None);
    };
    if filename.as_os_str() != resource.filename.as_str() {
        return Ok(None);
    }

    let name = profile_name.as_os_str().to_string_lossy().into_owned();
    if validate_name(&name).is_err() {
        return Ok(None);
    }
    // Stored resources must not themselves alias another file or profile.
    config::profile_resource(target, &name, resource)?;
    Ok(Some(name))
}
