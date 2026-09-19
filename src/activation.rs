use std::fs;
use std::path::{Path, PathBuf};

use crate::config;
use crate::error::{AppError, IoContext, Result};
use crate::fs_util;
use crate::profile;
use crate::targets::{ResourceSpec, TargetSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Active(String),
    NoFiles,
    Partial,
    Mixed,
    Unmanaged,
}

pub fn status(target: &TargetSpec) -> Result<Status> {
    status_with(target, |name| profile::is_complete(target, name))
}

fn status_with(
    target: &TargetSpec,
    is_complete: impl FnOnce(&str) -> Result<bool>,
) -> Result<Status> {
    let mut linked_profiles = Vec::new();
    let mut found = false;

    for resource in &target.resources {
        let link = config::active_resource(target, resource)?;
        let metadata = match fs::symlink_metadata(&link) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(AppError::io(&link, error)),
        };
        found = true;
        if !metadata.file_type().is_symlink() {
            return Ok(Status::Unmanaged);
        }
        match managed_link_profile(target, resource, &link)? {
            Some(name) => linked_profiles.push(name),
            None => return Ok(Status::Unmanaged),
        }
    }

    if !found {
        return Ok(Status::NoFiles);
    }
    let Some(name) = linked_profiles.first() else {
        return Ok(Status::Partial);
    };
    if linked_profiles.iter().any(|linked| linked != name) {
        return Ok(Status::Mixed);
    }

    let stored_resources = target
        .resources
        .iter()
        .filter(|resource| {
            config::profile_resource(target, name, resource).is_ok_and(|path| path.exists())
        })
        .count();
    if linked_profiles.len() != stored_resources || !is_complete(name)? {
        return Ok(Status::Partial);
    }
    Ok(Status::Active(name.clone()))
}

pub fn active_name(target: &TargetSpec) -> Result<Option<String>> {
    Ok(match status(target)? {
        Status::Active(name) => Some(name),
        _ => None,
    })
}

pub(crate) fn active_name_from_profiles(
    target: &TargetSpec,
    profiles: &[profile::ProfileInfo],
) -> Result<Option<String>> {
    let status = status_with(target, |name| {
        Ok(profiles
            .iter()
            .find(|profile| profile.name == name)
            .is_some_and(|profile| profile.complete))
    })?;
    Ok(match status {
        Status::Active(name) => Some(name),
        _ => None,
    })
}

pub fn switch(target: &TargetSpec, name: &str, force: bool) -> Result<bool> {
    profile::validate_name(name)?;
    if !profile::is_complete(target, name)? {
        return Err(AppError::IncompleteProfile(name.to_string()));
    }
    if active_name(target)?.as_deref() == Some(name) {
        return Ok(true);
    }

    LinkTransaction::prepare(target, name, force)?.commit()?;
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
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
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
    let Ok(relative) = target_path.strip_prefix(config::profiles_dir(target)?) else {
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
    Ok(profile::validate_name(&name).is_ok().then_some(name))
}

struct LinkTransaction {
    operations: Vec<LinkOperation>,
}

impl LinkTransaction {
    fn prepare(target: &TargetSpec, profile: &str, force: bool) -> Result<Self> {
        let mut operations = Vec::with_capacity(target.resources.len());
        for (index, resource) in target.resources.iter().enumerate() {
            let link = config::active_resource(target, resource)?;
            let source = config::profile_resource(target, profile, resource)?;
            let had_existing = match fs::symlink_metadata(&link) {
                Ok(_) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => return Err(AppError::io(&link, error)),
            };
            if had_existing && managed_link_profile(target, resource, &link)?.is_none() && !force {
                return Err(AppError::UnmanagedActivePath(link.display().to_string()));
            }
            operations.push(LinkOperation::new(
                index,
                link,
                source.exists().then_some(source),
                had_existing,
            ));
        }
        Ok(Self { operations })
    }

    fn commit(self) -> Result<()> {
        if let Err(error) = self.materialize() {
            self.clear_staging();
            return Err(error);
        }

        for (index, operation) in self.operations.iter().enumerate() {
            if let Err(error) = operation.apply() {
                for applied in self.operations[..index].iter().rev() {
                    applied.rollback();
                }
                self.clear_staging();
                return Err(error);
            }
        }
        for operation in &self.operations {
            operation.finish();
        }
        Ok(())
    }

    fn materialize(&self) -> Result<()> {
        for operation in &self.operations {
            operation.create_staging()?;
        }
        Ok(())
    }

    fn clear_staging(&self) {
        for operation in &self.operations {
            let _ = fs::remove_file(&operation.staging);
        }
    }
}

struct LinkOperation {
    link: PathBuf,
    source: Option<PathBuf>,
    staging: PathBuf,
    rollback: PathBuf,
    had_existing: bool,
}

impl LinkOperation {
    fn new(index: usize, link: PathBuf, source: Option<PathBuf>, had_existing: bool) -> Self {
        let suffix = format!(".cprof-{}-{index}", std::process::id());
        let mut staging = link.as_os_str().to_os_string();
        staging.push(format!("{suffix}.stage"));
        let mut rollback = link.as_os_str().to_os_string();
        rollback.push(format!("{suffix}.rollback"));
        Self {
            staging: PathBuf::from(staging),
            rollback: PathBuf::from(rollback),
            link,
            source,
            had_existing,
        }
    }

    fn create_staging(&self) -> Result<()> {
        if let Some(parent) = self.link.parent() {
            fs::create_dir_all(parent).with_path(parent)?;
        }
        let _ = fs::remove_file(&self.staging);
        let _ = fs::remove_file(&self.rollback);
        if let Some(source) = &self.source {
            fs_util::create_symlink(source, &self.staging)?;
        }
        Ok(())
    }

    fn apply(&self) -> Result<()> {
        if self.had_existing {
            fs::rename(&self.link, &self.rollback).with_path(&self.link)?;
        }
        if self.source.is_some()
            && let Err(error) = fs::rename(&self.staging, &self.link)
        {
            if self.had_existing {
                let _ = fs::rename(&self.rollback, &self.link);
            }
            return Err(AppError::io(&self.link, error));
        }
        Ok(())
    }

    fn rollback(&self) {
        let _ = fs::remove_file(&self.link);
        if self.had_existing {
            let _ = fs::rename(&self.rollback, &self.link);
        }
    }

    fn finish(&self) {
        if self.had_existing {
            let _ = fs::remove_file(&self.rollback);
        }
    }
}
