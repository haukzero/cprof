//! Inspect resource entries and plan concrete link changes without writing them.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{self, paths};
use crate::error::{ActivationError, AppError, IoContext, Result};
use crate::filesystem::{self, transaction::PathTransaction};
use crate::profile::validate_name;
use crate::targets::{ResourceSpec, TargetSpec};

#[derive(PartialEq, Eq)]
pub(super) enum State {
    Missing,
    Managed(String),
    Unmanaged,
}

pub(crate) struct LinkPlan {
    changes: Vec<LinkChange>,
}

enum LinkChange {
    Set {
        path: PathBuf,
        source: PathBuf,
        replace: bool,
    },
    Remove(PathBuf),
}

impl LinkPlan {
    /// Resource keys describe the profile after any pending replacement commits.
    pub(crate) fn activate(
        target: &TargetSpec,
        name: &str,
        resources: &BTreeSet<String>,
        force: bool,
    ) -> Result<Self> {
        validate_name(name)?;
        let mut changes = Vec::new();
        for resource in &target.resources {
            let path = config::active_resource(target, resource)?;
            let state = read(target, resource, &path)?;
            if state == State::Unmanaged && !force {
                return Err(ActivationError::UnmanagedPath(path.display().to_string()).into());
            }
            if resources.contains(&resource.key) {
                if matches!(&state, State::Managed(current) if current == name) {
                    continue;
                }
                changes.push(LinkChange::Set {
                    path,
                    source: config::profile_resource(target, name, resource)?,
                    replace: state != State::Missing,
                });
            } else if state != State::Missing {
                changes.push(LinkChange::Remove(path));
            }
        }
        Ok(Self { changes })
    }

    pub(crate) fn clear(target: &TargetSpec) -> Result<Self> {
        let mut changes = Vec::new();
        for resource in &target.resources {
            let path = config::active_resource(target, resource)?;
            if filesystem::path_exists(&path)? {
                changes.push(LinkChange::Remove(path));
            }
        }
        Ok(Self { changes })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub(crate) fn stage(&self, transaction: &mut PathTransaction) -> Result<()> {
        for change in &self.changes {
            match change {
                LinkChange::Set {
                    path,
                    source,
                    replace,
                } => {
                    transaction.stage_symlink(path, source, *replace)?;
                }
                LinkChange::Remove(path) => transaction.stage_remove(path)?,
            }
        }
        Ok(())
    }
}

pub(super) fn read(target: &TargetSpec, resource: &ResourceSpec, link: &Path) -> Result<State> {
    let metadata = match fs::symlink_metadata(link) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(State::Missing),
        Err(error) => return Err(AppError::io(link, error)),
    };
    if !metadata.file_type().is_symlink() {
        return Ok(State::Unmanaged);
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
        return Ok(State::Unmanaged);
    };
    let mut parts = relative.components();
    let (Some(profile_name), Some(filename), None) = (parts.next(), parts.next(), parts.next())
    else {
        return Ok(State::Unmanaged);
    };
    if filename.as_os_str() != resource.filename.as_str() {
        return Ok(State::Unmanaged);
    }

    let name = profile_name.as_os_str().to_string_lossy().into_owned();
    if validate_name(&name).is_err() {
        return Ok(State::Unmanaged);
    }
    // Stored resources must not themselves alias another file or profile.
    config::profile_resource(target, &name, resource)?;
    Ok(State::Managed(name))
}
