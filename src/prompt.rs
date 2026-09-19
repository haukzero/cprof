use std::io::IsTerminal;

use dialoguer::{Confirm, FuzzySelect};

use crate::activation;
use crate::error::{AppError, Result};
use crate::profile;
use crate::targets::TargetSpec;

pub fn select_profile(target: &TargetSpec, name: Option<String>, prompt: &str) -> Result<String> {
    match name {
        Some(name) => Ok(name),
        None => {
            let profiles = profile::list(target)?;
            if profiles.is_empty() {
                return Err(AppError::ProfileNotFound("(no profiles exist)".to_string()));
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
            let selection = FuzzySelect::new()
                .with_prompt(prompt)
                .items(&labels)
                .interact()
                .map_err(|e| AppError::Other(e.to_string()))?;
            Ok(profiles[selection].name.clone())
        }
    }
}

pub fn select_copy_source(target: &TargetSpec) -> Result<Option<String>> {
    let profiles = profile::list(target)?;
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
    let selection = FuzzySelect::new()
        .with_prompt("Copy from (type to search)")
        .items(&labels)
        .interact()
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok((selection > 0).then(|| profiles[selection - 1].name.clone()))
}

pub fn require_profile(target: &TargetSpec, name: &str) -> Result<()> {
    if !profile::exists(target, name)? {
        return Err(AppError::ProfileNotFound(name.to_string()));
    }
    Ok(())
}

pub fn confirm(prompt: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Err(AppError::ConfirmationRequired);
    }
    Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .map_err(|e| AppError::Other(e.to_string()))
}
