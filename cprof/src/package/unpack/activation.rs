use crate::error::{ProfileError, Result};
use crate::package::ActiveProfile;
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;

use super::ActiveChange;
use super::target::PlannedProfile;

pub(super) struct ActivationPlan {
    pub(super) change: ActiveChange,
    pub(super) links: activation::LinkPlan,
}

impl ActivationPlan {
    /// Plan against the profiles that will actually remain after conflict decisions.
    pub(super) fn prepare(
        target: &TargetSpec,
        desired: &ActiveProfile,
        profiles: &[PlannedProfile],
        removed: &[String],
    ) -> Result<Option<Self>> {
        let previous = activation::status(target)?;
        match desired {
            ActiveProfile::Unspecified => {
                // Legacy packages preserve activation unless its profile is removed.
                if !previous
                    .active_name()
                    .is_some_and(|name| removed.iter().any(|removed| removed == name))
                {
                    return Ok(None);
                }
            }
            ActiveProfile::None if previous == activation::Status::NoFiles => {
                return Ok(None);
            }
            ActiveProfile::None => {}
            ActiveProfile::Named(name) => {
                let resources = match profiles
                    .iter()
                    .find(|profile| profile.package.name == *name)
                {
                    Some(profile) => profile
                        .package
                        .resources
                        .iter()
                        .map(|resource| resource.spec.key.clone())
                        .collect(),
                    None => {
                        if !storage::is_complete(target, name)? {
                            return Err(ProfileError::Incomplete(name.clone()).into());
                        }
                        storage::resource_keys(target, name)?
                    }
                };
                let links = activation::LinkPlan::activate(target, name, &resources, true)?;
                if links.is_empty() && previous.active_name() == Some(name) {
                    return Ok(None);
                }
                return Ok(Some(Self {
                    change: ActiveChange::Activate {
                        previous,
                        profile: name.clone(),
                    },
                    links,
                }));
            }
        }
        Ok(Some(Self {
            change: ActiveChange::Clear { previous },
            links: activation::LinkPlan::clear(target)?,
        }))
    }
}
