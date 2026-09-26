//! Query, validate, and persist stored profiles and their resources.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexSet;

use crate::config::{self, paths};
use crate::error::{AppError, IoContext, ProfileError, Result, TargetError, TransactionError};
use crate::filesystem::{self, has_edit_draft, transaction::PathTransaction};
use crate::targets::{ResourceSpec, TargetSpec};

use super::{ProfileCounts, ProfileInfo, ProfileResource, validate_name};

pub fn exists(target: &TargetSpec, name: &str) -> Result<bool> {
    validate_name(name)?;
    Ok(config::profile_dir(target, name)?.is_dir())
}

pub(crate) fn require_exists(target: &TargetSpec, name: &str) -> Result<()> {
    if !exists(target, name)? {
        return Err(ProfileError::NotFound(name.to_string()).into());
    }
    Ok(())
}

pub(crate) fn has_edit_drafts(target: &TargetSpec, name: &str) -> Result<bool> {
    validate_name(name)?;
    for resource in &target.resources {
        let path = config::profile_resource(target, name, resource)?;
        if has_edit_draft(&path) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn is_complete(target: &TargetSpec, name: &str) -> Result<bool> {
    validate_name(name)?;
    let dir = config::profile_dir(target, name)?;
    if !dir.is_dir() {
        return Ok(false);
    }

    for spec in &target.resources {
        let path = config::profile_resource(target, name, spec)?;
        if !path.is_file() {
            if spec.required {
                return Ok(false);
            }
            continue;
        }
        if spec.validate(&fs::read(&path).with_path(&path)?).is_err() {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn list(target: &TargetSpec) -> Result<Vec<ProfileInfo>> {
    names(target)?
        .into_iter()
        .map(|name| {
            Ok(ProfileInfo {
                complete: is_complete(target, &name)?,
                name,
            })
        })
        .collect()
}

pub fn counts(target: &TargetSpec) -> Result<ProfileCounts> {
    let profiles = list(target)?;
    Ok(ProfileCounts {
        total: profiles.len(),
        incomplete: profiles.iter().filter(|profile| !profile.complete).count(),
    })
}

/// List profile names without reading their resources.
pub(crate) fn names(target: &TargetSpec) -> Result<Vec<String>> {
    let dir = config::profiles_dir(target)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in fs::read_dir(&dir).with_path(&dir)? {
        let entry = entry.with_path(&dir)?;
        if !entry.file_type().with_path(&entry.path())?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if validate_name(&name).is_ok() {
            names.push(name);
        }
    }
    names.sort_unstable();
    Ok(names)
}

/// Resolve command-line profile names and `*`/`?` patterns in input order.
pub fn resolve_names(target: &TargetSpec, patterns: &[String]) -> Result<Vec<String>> {
    let available = if patterns.iter().any(|pattern| has_wildcards(pattern)) {
        names(target)?
    } else {
        Vec::new()
    };
    let mut resolved = IndexSet::new();

    for pattern in patterns {
        if has_wildcards(pattern) {
            let mut matched = false;
            for name in &available {
                if matches_pattern(pattern, name) {
                    matched = true;
                    resolved.insert(name.clone());
                }
            }
            if !matched {
                return Err(ProfileError::NoMatches(pattern.clone()).into());
            }
        } else {
            resolved.insert(pattern.clone());
        }
    }
    Ok(resolved.into_iter().collect())
}

fn has_wildcards(pattern: &str) -> bool {
    pattern.contains(['*', '?'])
}

fn matches_pattern(pattern: &str, name: &str) -> bool {
    let (mut pattern, mut name) = (pattern, name);
    let (mut star_pattern, mut star_name) = (None, "");

    loop {
        match (pattern.chars().next(), name.chars().next()) {
            (None, None) => return true,
            (Some('*'), _) => {
                pattern = drop_first_char(pattern);
                star_pattern = Some(pattern);
                star_name = name;
            }
            (Some('?'), Some(_)) => {
                pattern = drop_first_char(pattern);
                name = drop_first_char(name);
            }
            (Some(pattern_char), Some(name_char)) if pattern_char == name_char => {
                pattern = drop_first_char(pattern);
                name = drop_first_char(name);
            }
            _ => match star_pattern {
                Some(retry_pattern) if !star_name.is_empty() => {
                    star_name = drop_first_char(star_name);
                    name = star_name;
                    pattern = retry_pattern;
                }
                _ => return false,
            },
        }
    }
}

fn drop_first_char(value: &str) -> &str {
    &value[value.chars().next().expect("non-empty string").len_utf8()..]
}

pub fn create(target: &TargetSpec, name: &str, copy_from: Option<&str>) -> Result<()> {
    validate_name(name)?;
    let dir = config::profile_dir(target, name)?;
    if dir.exists() {
        return Err(ProfileError::Exists(name.to_string()).into());
    }

    let source_dir = match copy_from {
        Some(source) => {
            if !exists(target, source)? {
                return Err(ProfileError::NotFound(source.to_string()).into());
            }
            Some(config::profile_dir(target, source)?)
        }
        None => None,
    };

    let mut transaction = PathTransaction::new();
    let staging = transaction.stage_directory(&dir, false)?;
    let result = target.resources.iter().try_for_each(|resource| {
        let destination = resource_path_in(&staging, resource)?;
        if let Some(source_dir) = &source_dir {
            let source_path = resource_path_in(source_dir, resource)?;
            if source_path.exists() {
                filesystem::write_file(
                    &destination,
                    &fs::read(&source_path).with_path(&source_path)?,
                )?;
            }
        } else {
            write_resource(&destination, resource, &resource.template)?;
        }
        Ok::<(), AppError>(())
    });
    if let Err(error) = result {
        return Err(transaction.cancel(error));
    }
    transaction.commit().map_err(|error| match error {
        AppError::Transaction(TransactionError::Conflict(_)) if dir.exists() => {
            ProfileError::Exists(name.to_string()).into()
        }
        error => error,
    })
}

pub fn delete(target: &TargetSpec, name: &str) -> Result<()> {
    validate_name(name)?;
    let dir = config::profile_dir(target, name)?;
    if !dir.is_dir() {
        return Err(ProfileError::NotFound(name.to_string()).into());
    }
    fs::remove_dir_all(&dir).with_path(&dir)?;
    Ok(())
}

pub fn replace(
    target: &TargetSpec,
    name: &str,
    resources: &[ProfileResource],
    overwrite: bool,
) -> Result<()> {
    let mut transaction = PathTransaction::new();
    if let Err(error) = stage_replace(&mut transaction, target, name, resources, overwrite) {
        return Err(transaction.cancel(error));
    }
    transaction.commit()
}

fn stage_replace(
    transaction: &mut PathTransaction,
    target: &TargetSpec,
    name: &str,
    resources: &[ProfileResource],
    overwrite: bool,
) -> Result<()> {
    validate_replacement(target, name, resources, overwrite)?;
    stage_validated_replace(transaction, target, name, resources, overwrite)
}

pub(crate) fn validate_replacement(
    target: &TargetSpec,
    name: &str,
    resources: &[ProfileResource],
    overwrite: bool,
) -> Result<()> {
    validate_name(name)?;
    let destination = config::profile_dir(target, name)?;
    if destination.exists() && !overwrite {
        return Err(ProfileError::Exists(name.to_string()).into());
    }
    validate_resources(target, name, resources)
}

pub(crate) fn stage_validated_replace(
    transaction: &mut PathTransaction,
    target: &TargetSpec,
    name: &str,
    resources: &[ProfileResource],
    overwrite: bool,
) -> Result<()> {
    let destination = config::profile_dir(target, name)?;
    let staging = transaction.stage_directory(&destination, overwrite)?;
    write_resources(&staging, resources)
}

pub(crate) fn stage_delete(
    transaction: &mut PathTransaction,
    target: &TargetSpec,
    name: &str,
) -> Result<()> {
    validate_name(name)?;
    let directory = config::profile_dir(target, name)?;
    if !directory.is_dir() {
        return Err(ProfileError::NotFound(name.to_string()).into());
    }
    transaction.stage_remove(&directory)
}

pub fn read(target: &TargetSpec, name: &str) -> Result<Vec<ProfileResource>> {
    validate_name(name)?;
    if !exists(target, name)? {
        return Err(ProfileError::NotFound(name.to_string()).into());
    }

    let mut resources = Vec::new();
    for spec in &target.resources {
        let path = config::profile_resource(target, name, spec)?;
        if !path.exists() {
            if spec.required {
                return Err(ProfileError::Incomplete(name.to_string()).into());
            }
            continue;
        }
        let content = fs::read(&path).with_path(&path)?;
        spec.validate(&content)?;
        resources.push(ProfileResource {
            spec: spec.clone(),
            content,
        });
    }
    Ok(resources)
}

pub fn resource_path(target: &TargetSpec, name: &str, resource: &ResourceSpec) -> Result<PathBuf> {
    validate_name(name)?;
    config::profile_resource(target, name, resource)
}

pub fn validate_resource_file(path: &Path, resource: &ResourceSpec) -> Result<()> {
    resource.validate(&fs::read(path).with_path(path)?)?;
    Ok(())
}

fn write_resources(dir: &Path, resources: &[ProfileResource]) -> Result<()> {
    for resource in resources {
        let path = resource_path_in(dir, &resource.spec)?;
        filesystem::write_file(&path, &resource.content)?;
    }
    Ok(())
}

fn resource_path_in(dir: &Path, resource: &ResourceSpec) -> Result<PathBuf> {
    paths::validate_filename(&resource.filename)?;
    paths::join_storage_under(dir, Path::new(&resource.filename))
}

fn validate_resources(
    target: &TargetSpec,
    name: &str,
    resources: &[ProfileResource],
) -> Result<()> {
    let mut keys = HashSet::new();
    for resource in resources {
        let expected = target.resource(&resource.spec.key)?;
        if expected.filename != resource.spec.filename || !keys.insert(resource.spec.key.clone()) {
            return Err(TargetError::InvalidResource(format!(
                "Invalid resource '{}' in profile '{name}'",
                resource.spec.key
            ))
            .into());
        }
        expected.validate(&resource.content)?;
    }
    if target
        .resources
        .iter()
        .any(|spec| spec.required && !keys.contains(&spec.key))
    {
        return Err(ProfileError::Incomplete(name.to_string()).into());
    }
    Ok(())
}

fn write_resource(path: &Path, spec: &ResourceSpec, content: &[u8]) -> Result<()> {
    spec.validate(content)?;
    filesystem::write_file(path, content)
}

#[cfg(test)]
mod tests {
    use super::matches_pattern;

    #[test]
    fn wildcard_patterns_match_expected_names() {
        assert!(matches_pattern("work-*", "work-codex"));
        assert!(matches_pattern("c?dex", "codex"));
        assert!(matches_pattern("*", "anything"));
        assert!(matches_pattern("a*?", "a*"));
        assert!(!matches_pattern("work-*", "personal"));
        assert!(!matches_pattern("c?dex", "coddex"));
        assert!(matches_pattern("?配置", "项配置"));
    }
}
