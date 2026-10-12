//! Inspect and switch active resource links for stored profiles.

use std::collections::BTreeSet;

use crate::config;
use crate::error::{ProfileError, Result};
use crate::filesystem::transaction::PathTransaction;
use crate::targets::TargetSpec;

use super::{ProfileInfo, storage, validate_name};

mod links;

pub(crate) use links::LinkPlan;

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

/// Inspect current links while reusing completeness from this operation's profile list.
pub(crate) fn status_from_profiles(
    target: &TargetSpec,
    profiles: &[ProfileInfo],
) -> Result<Status> {
    status_with(target, |name| {
        Ok(profiles
            .iter()
            .find(|profile| profile.name == name)
            .is_some_and(|profile| profile.complete))
    })
}

fn status_with(
    target: &TargetSpec,
    is_complete: impl FnOnce(&str) -> Result<bool>,
) -> Result<Status> {
    let mut linked_profiles = Vec::new();
    let mut linked_resources = BTreeSet::new();
    for resource in &target.resources {
        let link = config::active_resource(target, resource)?;
        match links::read(target, resource, &link)? {
            links::State::Missing => {}
            links::State::Managed(name) => {
                linked_profiles.push(name);
                linked_resources.insert(resource.key.clone());
            }
            links::State::Unmanaged => return Ok(Status::Unmanaged),
        }
    }

    let Some(name) = linked_profiles.first() else {
        return Ok(Status::NoFiles);
    };
    if linked_profiles.iter().any(|linked| linked != name) {
        return Ok(Status::Mixed);
    }

    if linked_resources != storage::resource_keys(target, name)? || !is_complete(name)? {
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
    let status = status_from_profiles(target, profiles)?;
    Ok(status.active_name().map(str::to_owned))
}

pub fn switch(target: &TargetSpec, name: &str, force: bool) -> Result<bool> {
    validate_name(name)?;
    if !storage::is_complete(target, name)? {
        return Err(ProfileError::Incomplete(name.to_string()).into());
    }
    let resources = storage::resource_keys(target, name)?;
    let links = LinkPlan::activate(target, name, &resources, force)?;
    // Empty profiles have no active links and cannot already be active.
    if !resources.is_empty() && links.is_empty() {
        return Ok(true);
    }
    PathTransaction::prepare(|transaction| links.stage(transaction))?.commit()?;
    Ok(false)
}

pub fn remove_profile_links(target: &TargetSpec, name: &str) -> Result<bool> {
    let links = LinkPlan::remove_profile(target, name)?;
    PathTransaction::prepare(|transaction| links.stage(transaction))?.commit()?;
    Ok(!links.is_empty())
}
