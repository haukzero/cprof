use std::io::{self, IsTerminal};
use std::sync::Arc;

use dialoguer::{Confirm, FuzzySelect, Input, MultiSelect};

use crate::error::{InteractionError, ProfileError, Result};
use crate::profile::{activation, storage};
use crate::targets::TargetSpec;

fn interact<T>(interaction: impl FnOnce() -> dialoguer::Result<T>) -> Result<T> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(InteractionError::InputRequired.into());
    }
    Ok(interaction().map_err(InteractionError::from)?)
}

pub(crate) fn input(prompt: &str) -> Result<String> {
    interact(|| Input::new().with_prompt(prompt).interact_text())
}

fn select_one(prompt: &str, items: &[String]) -> Result<usize> {
    interact(|| {
        FuzzySelect::new()
            .with_prompt(prompt)
            .items(items)
            .interact()
    })
}

fn select_many(prompt: &str, items: &[String]) -> Result<Vec<usize>> {
    interact(|| {
        MultiSelect::new()
            .with_prompt(prompt)
            .items(items)
            .interact()
    })
}

pub(crate) fn select_targets(targets: &[Arc<TargetSpec>]) -> Result<Vec<Arc<TargetSpec>>> {
    let labels = targets
        .iter()
        .map(|target| target.id.clone())
        .collect::<Vec<_>>();
    Ok(
        select_many("Select targets to pack (space to select/deselect)", &labels)?
            .into_iter()
            .map(|index| Arc::clone(&targets[index]))
            .collect(),
    )
}

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

pub(crate) fn confirm(prompt: &str) -> Result<bool> {
    interact(|| Confirm::new().with_prompt(prompt).default(false).interact())
}
