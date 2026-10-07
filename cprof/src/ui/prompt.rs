use crate::error::{ProfileError, Result};
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;

use super::interaction::select_one;
pub(crate) use super::interaction::{confirm, input};

pub(crate) fn select_profile(
    target: &TargetSpec,
    name: Option<String>,
    prompt: &str,
) -> Result<String> {
    match name {
        Some(name) => Ok(name),
        None => {
            let profiles = storage::list(target)?;
            if profiles.is_empty() {
                return Err(ProfileError::NotFound("(no profiles exist)".to_string()).into());
            }
            let active = activation::active_name_from_profiles(target, &profiles)?;
            let labels: Vec<String> = profiles
                .iter()
                .map(|p| {
                    let status = if active.as_deref() == Some(p.name.as_str()) {
                        " (active)"
                    } else if !p.complete {
                        " (incomplete)"
                    } else {
                        ""
                    };
                    format!("{}{}", p.name, status)
                })
                .collect();
            let selection = select_one(prompt, &labels)?;
            Ok(profiles[selection].name.clone())
        }
    }
}

pub(crate) fn select_copy_source(target: &TargetSpec) -> Result<Option<String>> {
    let profiles = storage::list(target)?;
    let active = activation::active_name_from_profiles(target, &profiles)?;
    let mut labels = vec!["Default template".to_string()];
    labels.extend(profiles.iter().map(|profile| {
        let status = if active.as_deref() == Some(profile.name.as_str()) {
            " (active)"
        } else if !profile.complete {
            " (incomplete)"
        } else {
            ""
        };
        format!("{}{}", profile.name, status)
    }));
    let selection = select_one("Copy from (type to search)", &labels)?;
    Ok((selection > 0).then(|| profiles[selection - 1].name.clone()))
}
