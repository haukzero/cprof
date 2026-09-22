use std::collections::HashSet;
use std::io::{Read, Seek};

use zip::ZipArchive;

use crate::error::{PackageError, ProfileError, Result};
use crate::profile::{self, ProfileResource};
use crate::targets::TargetSpec;

use super::{PackageProfile, manifest};

pub(super) struct Encoded {
    pub(super) manifest: Vec<manifest::ManifestProfile>,
    pub(super) payloads: Vec<(String, Vec<u8>)>,
}

pub(super) fn encode(
    target: &TargetSpec,
    profiles: &[PackageProfile],
    target_index: usize,
) -> Result<Encoded> {
    if profiles.is_empty() {
        return Err(
            PackageError::Invalid(format!("Target '{}' contains no profiles", target.id)).into(),
        );
    }

    let mut names = HashSet::new();
    let mut manifest_profiles = Vec::with_capacity(profiles.len());
    let mut payloads = Vec::new();
    for (profile_index, profile) in profiles.iter().enumerate() {
        profile::validate_name(&profile.name)?;
        if !names.insert(profile.name.as_str()) {
            return Err(
                PackageError::Invalid(format!("Duplicate profile '{}'", profile.name)).into(),
            );
        }
        let mut keys = HashSet::new();
        let mut resources = Vec::with_capacity(profile.resources.len());
        for resource in &profile.resources {
            let Some(resource_index) = target
                .resources
                .iter()
                .position(|spec| spec.key == resource.spec.key)
            else {
                return Err(PackageError::Invalid(format!(
                    "Invalid resource '{}' for target '{}'",
                    resource.spec.key, target.id
                ))
                .into());
            };
            let expected = &target.resources[resource_index];
            if expected.filename != resource.spec.filename || !keys.insert(expected.key.clone()) {
                return Err(PackageError::Invalid(format!(
                    "Invalid resource '{}' in profile '{}'",
                    resource.spec.key, profile.name
                ))
                .into());
            }
            expected.validate(&resource.content)?;
            resources.push(resource_index);
            payloads.push((
                payload_path(target_index, profile_index, resource_index),
                resource.content.clone(),
            ));
        }
        if target
            .resources
            .iter()
            .any(|spec| spec.required && !keys.contains(&spec.key))
        {
            return Err(ProfileError::Incomplete(profile.name.clone()).into());
        }
        manifest_profiles.push(manifest::ManifestProfile {
            name: profile.name.clone(),
            resources,
        });
    }
    Ok(Encoded {
        manifest: manifest_profiles,
        payloads,
    })
}

pub(super) fn decode<R: Read + Seek>(
    target: &TargetSpec,
    target_index: usize,
    manifest_target: &manifest::ManifestTarget,
    archive: &mut ZipArchive<R>,
) -> Result<Vec<PackageProfile>> {
    let mut names = HashSet::new();
    let mut profiles = Vec::with_capacity(manifest_target.profiles.len());
    for (profile_index, manifest_profile) in manifest_target.profiles.iter().enumerate() {
        profile::validate_name(&manifest_profile.name)?;
        if !names.insert(manifest_profile.name.clone()) {
            return Err(PackageError::Invalid(format!(
                "Duplicate profile '{}'",
                manifest_profile.name
            ))
            .into());
        }

        let mut resource_ids = HashSet::new();
        let mut resources = Vec::with_capacity(manifest_profile.resources.len());
        for &resource_index in &manifest_profile.resources {
            let spec = target.resources.get(resource_index).ok_or_else(|| {
                PackageError::Invalid(format!(
                    "Unknown resource index {resource_index} for target '{}'",
                    target.id
                ))
            })?;
            if !resource_ids.insert(resource_index) {
                return Err(PackageError::Invalid(format!(
                    "Duplicate resource '{}' in profile '{}'",
                    spec.key, manifest_profile.name
                ))
                .into());
            }

            let path = payload_path(target_index, profile_index, resource_index);
            let mut file = archive.by_name(&path).map_err(|error| {
                PackageError::Invalid(format!("Invalid package archive: {error}"))
            })?;
            let mut content = Vec::new();
            file.read_to_end(&mut content)?;
            spec.validate(&content)?;
            resources.push(ProfileResource {
                spec: spec.clone(),
                content,
            });
        }
        if target
            .resources
            .iter()
            .enumerate()
            .any(|(index, spec)| spec.required && !resource_ids.contains(&index))
        {
            return Err(ProfileError::Incomplete(manifest_profile.name.clone()).into());
        }
        profiles.push(PackageProfile::new(
            manifest_profile.name.clone(),
            resources,
        ));
    }
    if profiles.is_empty() {
        return Err(
            PackageError::Invalid(format!("Target '{}' contains no profiles", target.id)).into(),
        );
    }
    Ok(profiles)
}

fn payload_path(target_index: usize, profile_index: usize, resource_index: usize) -> String {
    format!("p/{target_index}/{profile_index}/{resource_index}")
}
